---
id: TC-070
title: A plugin's vault page renders as safe HTML with the wikilinks, embeds and links it resolved, and nothing Echo doesn't allow
priority: high
type: functional
requirements: [F-PLUG-53]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, client-half, markdown, links, security]
---

# TC-070: A plugin's vault page renders as safe HTML with the links it resolved

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie); the browser 1201 × 868
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the picker)
- A made-up vault image and a made-up plugin **Acme pages** (folders it may read, a host half and a window half) in a local marketplace `acme-local`. Its window half asks Echo for the page's destinations, resolves them — `Roadmap…` to its own route, `Ideas` to a missing page (`null`), every other link to itself, images to its folder route — renders the page and shows which missing page was clicked:

```bash
mkdir -p "$D/vault/assets"
echo 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==' | base64 -d > "$D/vault/assets/logo.png"
M="$D/market"; mkdir -p "$M/plugins/acme-pages"
echo '{"name":"acme-local","plugins":[{"id":"acme-pages","source":"./plugins/acme-pages"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-pages","name":"Acme pages","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js","uses":{"folders":{"what":"vaults"}}}' > "$M/plugins/acme-pages/echo-plugin.json"
cat > "$M/plugins/acme-pages/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('root', () => ctx.fs.folders()[0] || '')
}
JS
cat > "$M/plugins/acme-pages/client.js" <<'JS'
const MD = [
  '# Plan',
  'See [[Roadmap]], [[Roadmap|the roadmap]], [[Roadmap#Q3]] and [[Ideas|new idea]].',
  '![[logo.png]] ![remote](https://acme.example/r.png) ![inline](data:image/png;base64,iVBORw0KGgo=)',
  '[Docs](https://acme.example/docs) [Bad](javascript:alert(1)) [Other](#other:main) [Plain](http://acme.example/) [Thread](echo://thread/atlas)',
  '<b>raw</b> <img src=x onerror=alert(1)>',
  'line one',
  'line two',
  '`[[Code]]`',
].join('\n')
export function apply(ctx) {
  let html = '', clicked = '', dests = null
  ctx.call('root').then(async (root) => {
    dests = await ctx.markdown.links(MD)
    const links = {}, images = {}
    for (const l of dests.links) links[l] = l.startsWith('Roadmap') ? `#acme-pages:main/${encodeURIComponent(l)}` : l === 'Ideas' ? null : l
    for (const i of dests.images) images[i] = i.includes(':') ? i : `plugin/acme-pages/folder/${encodeURIComponent(root)}/assets/${i}`
    html = await ctx.markdown(MD, { breaks: true, links, images })
    ctx.render()
  })
  ctx.listen('click', (e) => { const m = e.target.closest && e.target.closest('[data-md-missing]'); if (m) { clicked = m.dataset.mdMissing; ctx.render() } })
  ctx.activities.add({ id: 'main', label: 'Acme pages', icon: '', render: () => `<h2>Acme pages</h2><pre id="acme-dests">${ctx.ui.esc(JSON.stringify(dests))}</pre><div id="acme-page">${html}</div><p id="acme-clicked">${ctx.ui.esc(clicked)}</p>` })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-pages --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-pages add` | `Installed Acme pages 0.1.0`; the vault's path |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-pages:main`; after a moment read the text of `#acme-dests` | The page shows "Acme pages"; the destinations each once, as written: links `Roadmap`, `Roadmap#Q3`, `Ideas`, `https://acme.example/docs`, `javascript:alert(1)`, `#other:main`, `http://acme.example/`, `echo://thread/atlas`; images `logo.png`, `https://acme.example/r.png`, `data:image/png;base64,iVBORw0KGgo=`; nothing of `Code` |
| 3 | Read `#acme-page`'s `innerHTML` and each `a`'s `href` attribute in it | Links with an href: `#acme-pages:main/Roadmap` (labels "Roadmap" and "the roadmap"), `#acme-pages:main/Roadmap%23Q3`, `https://acme.example/docs`, `echo://thread/atlas` — no other; "Bad", "Other" and "Plain" are text with no link; "new idea" is an `a.md-missing` with `data-md-missing="Ideas"` and no href; "raw" and the `<img src=x …>` show as text (`&lt;b&gt;`), no `script`, no `onerror` attribute; "line one" and "line two" on two lines (`<br>`); `[[Code]]` stays code |
| 4 | Read each `img` in `#acme-page`: its `src` attribute and `naturalWidth` | One image: `logo.png` from `plugin/acme-pages/folder/…/assets/logo.png`, loaded (`1`); "remote" and "inline" show as `[remote]` and `[inline]` text |
| 5 | Click "new idea" | `#acme-clicked` reads `Ideas`; the address is still `#acme-pages:main` |
| 6 | Click "the roadmap" | The address is `#acme-pages:main/Roadmap`; the page still shows "Acme pages" |

## Expected Final State
Acme pages is installed with the vault given to it; nothing changed in the vault.

## Teardown
- `echoctl plugin remove acme-pages --delete-data`; `echoctl plugin marketplace remove acme-local`

---
id: TC-060
title: A plugin's window half loads its own stylesheet (a path outside its package refused), keeps typed text while it redraws the same, and when switched off runs its cleanup and takes its stylesheet and window listener out
priority: medium
type: functional
requirements: [F-PLUG-43]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, stylesheet, cleanup]
---

# TC-060: A plugin's window half loads its stylesheet and cleans up after itself when switched off

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); CLI steps run in Bash after `. <dir>/env`; the browser 1201 × 868
- A made-up local marketplace `acme-local` with one plugin, **Acme styled**, that has only a window half. The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-styled/styles"
echo '{"name":"acme-local","plugins":[{"id":"acme-styled","source":"./plugins/acme-styled"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-styled","name":"Acme styled","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js"}' > "$M/plugins/acme-styled/echo-plugin.json"
echo '.acme-hello { color: rgb(1, 2, 3); }' > "$M/plugins/acme-styled/styles/acme.css"
cat > "$M/plugins/acme-styled/client.js" <<'EOF'
export function apply(ctx) {
  let resized = 0
  const box = () => document.getElementById('acme-box')
  const draw = () => `<p class="acme-hello">Hello from Acme styled</p><input aria-label="Acme note"><p>Resized ${resized} times</p>`
  ctx.panels.add({ id: 'box', label: 'Acme styled', render: () => `<div id="acme-box">${draw()}</div>` })
  // The same HTML twice a second: patch leaves the box as it is.
  const tick = setInterval(() => ctx.ui.patch(box(), draw()), 500)
  ctx.listenWindow('resize', () => { resized++; document.body.dataset.acmeResized = String(resized); ctx.ui.patch(box(), draw()) })
  ctx.onUnload(() => { clearInterval(tick); document.body.dataset.acmeUnloaded = 'yes' })
  ctx.ui.stylesheet('styles/acme.css').then(() => { document.body.dataset.acmeStyle = 'loaded' })
  ctx.ui.stylesheet('../outside.css').catch((e) => { document.body.dataset.acmeRefused = e.message })
}
EOF
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-styled --yes`; reload `{{base_url}}/` | `Installed Acme styled 0.1.0`; the page loads |
| 2 | Read `document.body.dataset.acmeStyle`, `document.body.dataset.acmeRefused` and `document.querySelectorAll('link[data-plugin="acme-styled"]').length` | `loaded`; `Not a stylesheet of the plugin: ../outside.css`; `1` — the link's `href` ends with `plugin/acme-styled/styles/acme.css?v=…` |
| 3 | Open the right panel with the Inbox button (`[data-panel-flip="needs"]`; if the panel is already open, leave it); click the tab "Acme styled" (`[data-panel-show="acme-styled:box"]`) | The panel shows "Hello from Acme styled" and "Resized 0 times"; the computed colour of "Hello from Acme styled" is `rgb(1, 2, 3)` |
| 4 | Click the field "Acme note", type `remember me`, wait 2 seconds | The field still holds `remember me` and keeps the focus (the plugin patched the box with the same HTML four times; nothing was rewritten) |
| 5 | Resize the browser to 1100 × 868 | The panel reads "Resized N times" with N ≥ 1 (one per resize event the browser sent); `document.body.dataset.acmeResized` is the same N |
| 6 | Navigate to `{{base_url}}/#settings/plugins`, click the switch "Acme styled on" in the detail | It is off (`aria-checked="false"`); `document.body.dataset.acmeUnloaded` is `yes`; `document.querySelectorAll('link[data-plugin="acme-styled"]').length` is `0`; the right panel has no tab "Acme styled" |
| 7 | Resize the browser back to 1201 × 868; then read `document.body.dataset.acmeResized` and `document.querySelectorAll('.acme-hello').length` | `acmeResized` is still step 5's N (its window listener is gone with it: it didn't hear this resize); `0` |

## Expected Final State
Acme styled is installed and off; nothing of it is left in the page.

## Teardown
- `echoctl plugin remove acme-styled --delete-data`; `echoctl plugin marketplace remove acme-local`

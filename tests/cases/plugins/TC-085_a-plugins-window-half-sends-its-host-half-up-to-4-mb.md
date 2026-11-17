---
id: TC-085
title: A plugin's window half sends its host half a call and a page far past 64 KB, up to 4 MB, and a call over 4 MB is refused
priority: high
type: functional
requirements: [F-PLUG-62]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, call]
---

# TC-085: A plugin's window half sends its host half up to 4 MB

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie); the browser 1201 × 868
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- A made-up plugin **Acme big** (a host half and a window half) in a local marketplace `acme-local`. Its window half sends its host half a long Cyrillic text, renders a long Cyrillic page, and sends a call over 4 MB, and shows each answer:

```bash
M="$D/market"; mkdir -p "$M/plugins/acme-big"
echo '{"name":"acme-local","plugins":[{"id":"acme-big","source":"./plugins/acme-big"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-big","name":"Acme big","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js"}' > "$M/plugins/acme-big/echo-plugin.json"
cat > "$M/plugins/acme-big/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('size', (a) => [a.text.length, a.text.slice(0, 6)])
}
JS
cat > "$M/plugins/acme-big/client.js" <<'JS'
export function apply(ctx) {
  // Far past what a request's address holds (64 KB). Shown in #acme-big when each answered.
  const big = { call: '…', page: '…', over: '…' }
  const show = (k) => [(v) => { big[k] = v; ctx.render() }, (e) => { big[k] = `error: ${e.message}`; ctx.render() }]
  ctx.call('size', { text: 'Привіт '.repeat(140000) }).then((r) => r.join(' ')).then(...show('call'))
  ctx.markdown('Привіт '.repeat(70000)).then((h) => `${h.split('Привіт').length - 1} words`).then(...show('page'))
  ctx.call('size', { text: 'ж'.repeat(2200000) }).then((r) => r.join(' ')).then(...show('over'))
  ctx.activities.add({ id: 'main', label: 'Acme big', icon: '', render: () => `<h2>Acme big</h2><p id="acme-big">${ctx.ui.esc(`${big.call} | ${big.page} | ${big.over}`)}</p>` })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-big --yes` | `Installed Acme big 0.1.0` |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-big:main`; after a moment read the text of `#acme-big` | `980000 Привіт | 70000 words | error: too much for one plugin call: 4 MB at most` — the host half got all 980,000 letters of the long call, the long page came back whole, the call over 4 MB was refused; no `…` left |

## Expected Final State
Acme big is installed and on.

## Teardown
- `echoctl plugin remove acme-big --delete-data`; `echoctl plugin marketplace remove acme-local`

## Notes
- A switched-off or unknown plugin, a GET and a body that isn't JSON are refused by the routes themselves (Rust tests in `window::plugins`); the native window takes the same POST through wry — checked by hand at a release.

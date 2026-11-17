---
id: TC-068
title: A plugin's window half reaches an address the user listed over WebSocket and fetch only once it is listed — the page loads again by itself, on the same route, when the list changes, not while the user types, and not otherwise
priority: high
type: functional
requirements: [F-PLUG-51]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, client-half, network, websocket, csp, settings]
---

# TC-068: A plugin's window half talks to the user's voice server over WebSocket once its address is listed

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set small`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); the browser 1201 × 868
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`. `P` is a free port (e.g. the window's port + 10)
- A made-up voice server on this computer: WebSocket JSON-RPC (it answers each request with its method and params) and `GET /ping` (CORS open), each connection a line in its log:

```bash
nohup python3 tests/fixtures/ws_echo.py "$P" "$D/ws.log" > "$D/ws.out" 2>&1 & echo $! > "$D/ws.pid"
```
- A made-up local marketplace `acme-local` with **Acme voice**, a window half only; its manifest lists the setting `voice` in `network.hosts_from`. When it loads it opens a WebSocket to `ws://127.0.0.1:P/rpc`, sends one JSON-RPC request and fetches `http://127.0.0.1:P/ping`, writing what happened on the page (`document.body.dataset.acmeWs`, `acmeFetch`). The only files this case writes, in the run's scratch folder (the heredoc is unquoted: `$P` is the port):

```bash
M="$D/market-voice"; mkdir -p "$M/plugins/acme-voice"
echo '{"name":"acme-local","plugins":[{"id":"acme-voice","source":"./plugins/acme-voice"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-voice","name":"Acme voice","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js","uses":{"network":{"hosts_from":["voice"]}},"settings":[{"key":"voice","label":"Voice server","type":"string"}]}' > "$M/plugins/acme-voice/echo-plugin.json"
cat > "$M/plugins/acme-voice/client.js" <<EOF
export function apply(ctx) {
  const say = (k, v) => { document.body.dataset[k] = v }
  say('acmeWs', 'trying'); say('acmeFetch', 'trying')
  let ws
  try { ws = new WebSocket('ws://127.0.0.1:$P/rpc') } catch (e) { say('acmeWs', 'blocked: ' + e.name); ws = null }
  if (ws) {
    ws.onopen = () => ws.send(JSON.stringify({ jsonrpc: '2.0', id: 7, method: 'speak', params: { text: 'hello' } }))
    ws.onmessage = (m) => { const r = JSON.parse(m.data); say('acmeWs', 'answer ' + r.id + ' ' + r.result.method + ' ' + r.result.params.text); ws.close() }
    ws.onerror = () => say('acmeWs', 'blocked')
  }
  fetch('http://127.0.0.1:$P/ping').then((r) => r.json()).then((j) => say('acmeFetch', 'ok ' + j.ok), () => say('acmeFetch', 'blocked'))
  ctx.onUnload(() => { if (ws) ws.close() })
}
EOF
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, the addresses the user lists)

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme voice (`acme-voice`) 0.1.0, client half only, `uses.network.hosts_from: ["voice"]`, setting `voice` (string, empty) |
| Voice server | `ws://127.0.0.1:P/rpc` (JSON-RPC: `speak` → `{"method":"speak","params":{"text":"hello"}}`), `http://127.0.0.1:P/ping` → `{"ok": true}` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-voice --yes` | Among its lines `• Talks to the addresses you list in its setting “voice” (wildcards allowed, plain http too; its window part also over WebSocket) (new)`; then `Installed Acme voice 0.1.0` |
| 2 | Navigate to `{{base_url}}/#settings/plugins` and reload it; wait 3 s; read `document.body.dataset.acmeWs`, `document.body.dataset.acmeFetch` and `document.querySelector('meta[name="echo-net"]').content`; then `cat "$D/ws.log"` | `blocked` and `blocked`: nothing listed, so the page's policy refuses both (the console says `Connecting to 'ws://127.0.0.1:P/rpc' violates the following Content Security Policy directive: "connect-src 'self' echo: http://echo.localhost"`, and the same for `/ping`); the meta holds a short key; the server's log is empty (or missing): no connection reached it |
| 3 | Set a mark that only a reload clears: `window.__acmeMark = 'kept'`; wait 6 s (three of the window's 2 s reads of its state); read `window.__acmeMark` | `kept`: nothing listed changed, so the page did not load again |
| 4 | `echoctl plugin set acme-voice voice "http://127.0.0.1:$P"`; then `curl -s -D - -o /dev/null {{base_url}}/ \| grep -i '^content-security-policy'` | It saves and prints one note, `Note: Echo's window is one page: while this plugin is on, the window part of any plugin you have on can reach these addresses too` (no plain-http warning: 127.0.0.1 is the user's own computer); the header holds `connect-src 'self' echo: http://echo.localhost http://127.0.0.1:P ws://127.0.0.1:P` and `media-src` the same, and `img-src`, `style-src`, `script-src` name no outside address |
| 5 | Within 6 s, without touching the page, read `window.__acmeMark`, `location.hash`, `document.body.dataset.acmeWs` and `document.body.dataset.acmeFetch` (wait for `acmeWs` to stop being `trying`); then `cat "$D/ws.log"` | `undefined` (the page loaded again by itself); the hash is still `#settings/plugins`; `answer 7 speak hello` and `ok true`; the log has `ws /rpc`, `rpc speak` and `get /ping` |
| 6 | `wc -l < "$D/ws.log"` (N: what step 5's reload connected). Set the mark again (`window.__acmeMark = 'kept'`); on the tab "Installed" click "Acme voice", click into its field "Voice server" and type `x` at the end, without leaving the field; `echoctl plugin set acme-voice voice ""`; wait 6 s; read `window.__acmeMark` | `kept`: the user is typing, so the page waits for them |
| 7 | Empty the field (select all, Backspace) and press Tab; within 6 s read `window.__acmeMark`, `location.hash`, `document.body.dataset.acmeWs` and `document.body.dataset.acmeFetch`; look at the right of the tab "Installed"; then `wc -l < "$D/ws.log"` | `undefined`: once the field is left (the empty list saved, as the CLI set it), the page loaded again; still `#settings/plugins` with the detail of "Acme voice" open; `blocked` and `blocked` again; the log still has N lines: nothing new reached the server |

## Expected Final State
Acme voice is installed with no address listed; its window half can't reach the voice server; the page's policy names no outside address.

## Teardown
- `kill $(cat "$D/ws.pid")`
- `echoctl plugin remove acme-voice --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-PLUG-51), not from the code.
- The warning for plain http to a name outside the user's network (F-PLUG-11) is `plugins/TC-018`'s.
- The served window sends the page's policy in its `Content-Security-Policy` header as the native window does, so the browser enforces it the same; the native window's own origin (`echo://localhost` on macOS, `http://echo.localhost` on Windows) is checked by the Rust tests only.
- A `*` number of an IPv4 address (`http://127.0.0.*:P`) isn't added to the page's policy (CSP can't say it): its row says the window part can't reach it. The Rust tests in `plugins::addresses` hold the full mapping (`*.local`, ports, https → wss).

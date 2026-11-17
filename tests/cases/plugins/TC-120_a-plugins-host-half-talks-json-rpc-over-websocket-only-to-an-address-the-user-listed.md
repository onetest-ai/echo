---
id: TC-120
title: A plugin's host half talks JSON-RPC over WebSocket to an address the user listed, and to nothing else
priority: high
type: functional
requirements: [F-PLUG-80]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, websocket, cli]
---

# TC-120: A plugin's host half talks to the user's voice server over WebSocket once its address is listed

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run: `tests/fixtures/serve.sh up <dir> 48120 --set small`. Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- A made-up voice server on this computer, plain (port 48121) — WebSocket JSON-RPC (it answers each request with its method and params), each connection a line in its log:

```bash
nohup python3 tests/fixtures/ws_echo.py 48121 "$D/ws.log" > "$D/ws.out" 2>&1 & echo $! > "$D/ws.pid"
```
- A made-up local marketplace `acme-net` with **Acme relay**, a host half only (`main.js`); its manifest lists the setting `voice` in `network.hosts_from` and names `api.acme.example` in `network.hosts`. Its commands: `rpc` opens a WebSocket to `url`, sends one JSON-RPC request (`speak`, `{text}`) and answers what came back; `stream` reads `url` as it comes and answers its status, content type, pieces and bytes; `many` opens `n` WebSockets and answers how each went; `get` is `ctx.http.fetch(url, opts)`. The only files this case writes, in the run's scratch folder:

```bash
M="$D/market-net"; mkdir -p "$M/plugins/acme-relay"
echo '{"name":"acme-net","plugins":[{"id":"acme-relay","source":"./plugins/acme-relay"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-relay","name":"Acme relay","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"network":{"hosts":["api.acme.example"],"hosts_from":["voice"]}},"settings":[{"key":"voice","label":"Voice server","type":"string"}]}' > "$M/plugins/acme-relay/echo-plugin.json"
cat > "$M/plugins/acme-relay/main.js" <<'EOF'
export function apply(ctx) {
  ctx.command('rpc', async (a) => {
    const ws = await ctx.net.websocket(a.url)
    const answer = await new Promise((res) => { ws.onMessage = (d) => res(JSON.parse(d)); ws.send(JSON.stringify({ jsonrpc: '2.0', id: 7, method: 'speak', params: { text: a.text || 'hello' } })) })
    ws.close(1000, 'done')
    return answer
  })
  ctx.command('stream', (a) => new Promise((res) => {
    const parts = []
    const s = ctx.http.stream(a.url, {}, (t) => parts.push(t))
    s.done.then((d) => res({ status: s.status, type: s.headers['content-type'], parts, bytes: d.bytes }), (e) => res({ error: e.message }))
  }))
  ctx.command('many', async (a) => { const out = []; for (let i = 0; i < a.n; i++) { try { await ctx.net.websocket(a.url); out.push('open') } catch (e) { out.push(e.message) } } return out })
  ctx.command('get', (a) => ctx.http.fetch(a.url, a.opts || {}))
}
EOF
echoctl plugin marketplace add "$M" --name acme-net
echoctl plugin install acme-relay --yes
```

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, the addresses the user lists)

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme relay (`acme-relay`) 0.1.0, host half only, setting `voice` (empty) |
| Voice server | `ws://127.0.0.1:48121/rpc` (JSON-RPC: `speak` → `{"method":"speak","params":{"text":…}}`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay rpc '{"url":"ws://127.0.0.1:48121/rpc"}'`; then `cat "$D/ws.log"` | It fails (exit 1) with `acme-relay may reach only the hosts its manifest names (api.acme.example) and the addresses you list in its settings ()`; the log is empty or missing: nothing reached the server |
| 2 | `echoctl plugin call acme-relay rpc '{"url":"wss://api.acme.example/rpc"}'` | It fails with `acme-relay opens a WebSocket only to the addresses you list in its settings`: a host the manifest names is for https requests, not a WebSocket |
| 3 | `echoctl plugin set acme-relay voice "http://127.0.0.1:48121"`; then `echoctl plugin call acme-relay rpc '{"url":"ws://127.0.0.1:48121/rpc","text":"привіт"}'`; then `cat "$D/ws.log"` | It saves; the call prints `{"id": 7, "jsonrpc": "2.0", "result": {"method": "speak", "params": {"text": "привіт"}}}` (key order may differ); the log has `ws /rpc` and `rpc speak` |
| 4 | `echoctl plugin call acme-relay rpc '{"url":"ws://127.0.0.1:48122/rpc"}'` | It fails with `may reach only`: another port than the one listed |

## Expected Final State
Acme relay is installed with `http://127.0.0.1:48121` listed; the voice server's log holds one connection.

## Teardown
- Kept for TC-121 to TC-124 in S-12. On its own: `kill $(cat "$D/ws.pid")`; `echoctl plugin remove acme-relay --delete-data`; `echoctl plugin marketplace remove acme-net`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from the feature's text (F-PLUG-80), not from the code. The host half runs here in the terminal's own process (`echoctl plugin call`), as in the app.
- A binary message, a file sent as one, the close code heard and the 16 MB message cap are proven by the Rust tests (`jshost::net`).

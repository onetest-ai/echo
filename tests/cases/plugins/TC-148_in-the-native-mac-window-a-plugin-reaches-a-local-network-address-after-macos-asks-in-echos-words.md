---
id: TC-148
title: In the native macOS window a plugin's window half reaches a .local address the user listed, after macOS asks once with Echo's own Local Network text
priority: high
type: functional
requirements: [F-PLUG-51]
module: plugins
run: person
platform: macos
data: none
tags: [plugins, client-half, network, privacy, native-window, release]
---

# TC-148: The native Mac window reaches a listed address on the local network

**Module:** Plugins | **Priority:** High | **Type:** Functional — a person, at the next release, on macOS 15 or later

## Preconditions
- The release build (signed, from `scripts/package-macos.sh`) installed as the user's Echo; Echo never asked for Local Network before (System Settings → Privacy & Security → Local Network: Echo not listed, or `tccutil reset LocalNetwork ai.onetest.echo` and Echo restarted)
- A second computer on the same network, its `.local` name known (`scutil --get LocalHostName` there; below `acme-box`), serving any page: `python3 -m http.server 8765 --bind 0.0.0.0` there
- On this Mac, a made-up local marketplace `acme-local` with **Acme lan**, a window half only that fetches the listed address when it loads and writes the outcome on the page (`document.body.dataset.acmeLan`). `no-cors`: the reply is opaque, so only "reached" or "blocked" is told:

```bash
M="$(mktemp -d)/market-lan"; mkdir -p "$M/plugins/acme-lan"
echo '{"name":"acme-local","plugins":[{"id":"acme-lan","source":"./plugins/acme-lan"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-lan","name":"Acme lan","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js","uses":{"network":{"hosts_from":["server"]}},"settings":[{"key":"server","label":"Server","type":"string"}]}' > "$M/plugins/acme-lan/echo-plugin.json"
cat > "$M/plugins/acme-lan/client.js" <<'EOF'
export function apply(ctx) {
  document.body.dataset.acmeLan = 'trying'
  fetch('http://acme-box.local:8765/', { mode: 'no-cors' }).then(() => { document.body.dataset.acmeLan = 'reached' }, (e) => { document.body.dataset.acmeLan = 'blocked: ' + e.name })
}
EOF
echoctl plugin marketplace add "$M" --name acme-local
echoctl plugin install acme-lan --yes
```

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin set acme-lan server http://acme-box.local:8765`; open the Echo window | macOS asks once whether Echo may find and connect to devices on the local network, under Echo's text "Echo connects to computers on your network only where you point it: a model or speech server you connected, or an address you listed for a plugin." |
| 2 | Allow; quit Echo from the tray and start it again; open the window; in the developer tools' console (dev build: right click → Inspect; on the release build, the second computer's `http.server` log) check `document.body.dataset.acmeLan` | `reached`; the second computer's log shows a `GET /` from this Mac; System Settings → Privacy & Security → Local Network lists Echo, on |
| 3 | Turn Echo off in that list; restart Echo; open the window | No `GET /` reaches the second computer (the page says `blocked: TypeError` where the console can be read); Echo itself works as before. Turn Echo on again |

## Expected Final State
Echo allowed on the local network; Acme lan removed.

## Teardown
- `echoctl plugin remove acme-lan --delete-data`; `echoctl plugin marketplace remove acme-local`; stop `http.server` on the second computer

## Notes
- Written from the feature's text (F-PLUG-51) and #697, not from the code.
- A person's check: the system's Local Network prompt and its list only show for the signed app on macOS 15 or later, and loopback (`127.0.0.1`, the served window's cases such as `plugins/TC-068`) never raises it.
- A WebSocket (`ws://`) and the host half's connections to a listed address go through the same permission; this case fetches only because a plain `http.server` answers that.
- Echo declares no `NSBonjourServices`: it browses and advertises no Bonjour service, and a `.local` name is resolved by the system's own resolver.

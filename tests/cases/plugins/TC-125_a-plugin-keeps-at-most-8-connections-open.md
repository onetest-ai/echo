---
id: TC-125
title: A plugin keeps at most 8 connections open at once — the 9th is refused saying so
priority: medium
type: functional
requirements: [F-PLUG-84]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, websocket, caps, cli]
---

# TC-125: A plugin opens a 9th WebSocket and is told it has 8 open

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-12: kept from that stage): Acme relay installed, the voice server on 48121, every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- Its address listed: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `N=$(grep -c "^ws " "$D/ws.log")`; `echoctl plugin call acme-relay many '{"url":"ws://127.0.0.1:48121/rpc","n":9}'`; then `echo $(( $(grep -c "^ws " "$D/ws.log") - N ))` | Eight `"open"`, then `"acme-relay has 8 connections open: close one first"`; `8`: the 9th never reached the server |

## Expected Final State
Unchanged; the terminal's process ended, and its connections with it.

## Teardown
- `kill $(cat "$D/ws.pid")`; `echoctl plugin remove acme-relay --delete-data`; `echoctl plugin marketplace remove acme-net`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from the feature's text (F-PLUG-84). Connections closed when the plugin is switched off, updated or removed (in the app's process, which runs the plugins' host halves) and the 16 MB message cap are proven by the Rust test `jshost::net::tests::a_plugin_talks_json_rpc_over_a_websocket_only_to_an_address_the_user_listed_and_loses_it_when_switched_off`.

---
id: TC-121
title: A plugin's host half reads an event stream from an address the user listed as it comes, piece by piece, no letter cut
priority: high
type: functional
requirements: [F-PLUG-81]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, sse, cli]
---

# TC-121: A plugin's host half reads the voice server's event stream as it comes

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-12: kept from that stage): Acme relay installed, the voice server on 48121 (`GET /events`: `data: one`, `data: два`, `data: three`, 300 ms apart), every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- Its address listed: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121"` (in S-12: TC-120's step 3 did it)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay stream '{"url":"http://127.0.0.1:48124/events"}'` | It fails with `may reach only`: port 48124 isn't listed, and nothing is read |
| 2 | `time echoctl plugin call acme-relay stream '{"url":"http://127.0.0.1:48121/events"}'` | It prints `status` 200, `type` `text/event-stream`, `parts` three pieces in order — `data: one\n\n`, `data: два\n\n`, `data: three\n\n` (each a piece of its own: the server sent them 300 ms apart) — and `bytes` 38; it took at least 0.6 s (it waited for the stream's end) |
| 3 | `grep -c "get /events" "$D/ws.log"` | `1`: the refused call never reached the server |

## Expected Final State
Unchanged: Acme relay installed, its address listed.

## Teardown
- Kept for TC-122 in S-12. On its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-81). Cancelling a stream that never ends (the server sees the client go), a letter cut across two pieces of the network, and a stream to an https address with a trusted key are the Rust tests' (`jshost::net`).

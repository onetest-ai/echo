---
id: TC-127
title: A certificate the user no longer trusts (`echoctl plugin untrust`) is refused again, its key kept as met
priority: high
type: functional
requirements: [F-PLUG-82]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, tls, pinning, cli]
---

# TC-127: The user stops trusting their voice server's certificate

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- TC-122's preconditions and its step 6 (in S-39: TC-126, which leaves certificate B shown and trusted): Acme relay installed, the voice server with TLS on 48122, its certificate's key trusted; every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay rpc '{"url":"wss://127.0.0.1:48122/rpc"}'` | It prints the JSON-RPC answer (`"id": 7`): trusted |
| 2 | `echoctl plugin untrust acme-relay 127.0.0.1:48122`; then `echoctl plugin untrust acme-relay 127.0.0.1:48122` again | `No longer trusted at 127.0.0.1:48122 for acme-relay`; then `Nothing was trusted at 127.0.0.1:48122 for acme-relay` |
| 3 | The `rpc` call of step 1 again; then `echoctl plugin certs acme-relay` | It fails with `the certificate at 127.0.0.1:48122 isn't one this computer trusts (its key: sha256//…)`, naming the key the server shows; the list has `127.0.0.1:48122` with `pinned` null and that key as `seen` |

## Expected Final State
Acme relay trusts no certificate; Echo keeps the server's key as met.

## Teardown
- `kill $(cat "$D/wss.pid")`. In S-39 the rest is kept for TC-123 (it starts its own server). On its own: `echoctl plugin untrust acme-relay 127.0.0.1:48122`; TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-82). "Forget" in the window does the same: TC-123.

---
id: TC-122
title: A server with a self-signed certificate is reached over wss and https only once the user trusted its key with the CLI
priority: high
type: functional
requirements: [F-PLUG-82]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, tls, pinning, cli]
---

# TC-122: The user trusts their voice server's self-signed certificate from the terminal

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-39: kept from that stage): Acme relay installed, every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- The voice server with TLS on 48122, showing the made-up self-signed certificate A (`tests/fixtures/certs/voice-a.pem`; no system trusts it):

```bash
nohup python3 tests/fixtures/ws_echo.py 48122 "$D/wss.log" --tls tests/fixtures/certs/voice-a.pem tests/fixtures/certs/voice-a.key > "$D/wss.out" 2>&1 & echo $! > "$D/wss.pid"
```
- Its address listed with the plain one: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121, https://127.0.0.1:48122"`

## Test Data

| Field | Value |
|---|---|
| Key of certificate A | `sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay rpc '{"url":"wss://127.0.0.1:48122/rpc"}'` | It fails with `the certificate at 127.0.0.1:48122 isn't one this computer trusts (its key: sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=): if it is your own server's, trust it in Settings → Plugins → Acme relay, or with \`echoctl plugin trust acme-relay 127.0.0.1:48122\`` |
| 2 | `echoctl plugin call acme-relay get '{"url":"https://127.0.0.1:48122/ping"}'`; then `cat "$D/wss.log" 2>/dev/null | grep -c "^ws\|^rpc\|^get"` | It fails the same way, naming the same key; `0`: no request went over |
| 3 | `echoctl plugin certs acme-relay` | `[{"address": "127.0.0.1:48122", "changed": false, "pinned": null, "seen": "sha256//Xy96…Flc="}]` (the full key of A) |
| 4 | `ECHOCTL_HEADLESS=1 echoctl plugin trust acme-relay 127.0.0.1:48122`; then `echoctl plugin certs acme-relay` | It fails with `plugins are installed, updated and changed by you, not from inside a run Echo started`; the list is as in step 3 |
| 5 | `echoctl plugin trust acme-relay 127.0.0.1:1` | It fails with `127.0.0.1:1 isn't an https address you listed for Acme relay: list it in its settings first` |
| 6 | `echoctl plugin trust acme-relay 127.0.0.1:48122`; then the calls of steps 1 and 2 again | `Trusted 127.0.0.1:48122 for acme-relay: sha256//Xy96…Flc=`; the `rpc` call prints the JSON-RPC answer (`"id": 7`, `"method": "speak"`); the `get` call prints `{"body": "{\"ok\": true}", "status": 200}` |

## Expected Final State
Acme relay trusts certificate A's key at 127.0.0.1:48122; the server there shows certificate A.

## Teardown
- In S-39 the server and the trust are kept for TC-126. On its own: `kill $(cat "$D/wss.pid")`; `echoctl plugin untrust acme-relay 127.0.0.1:48122`; TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-82). The key is the SHA-256 of the certificate's public key, as curl's `--pinnedpubkey` takes it: `openssl x509 -in tests/fixtures/certs/voice-a.pem -pubkey -noout | openssl pkey -pubin -outform der | openssl dgst -sha256 -binary | base64`.
- That a plugin can't write a pin through its store is proven by the Rust tests (`jshost::net`); that `trust` without `--key` is refused once Echo met more than one key at the address (the user names the one they mean), and that a plugin id that isn't one names no file, by `plugins::pins`'s.
- A changed key: TC-126; untrusting: TC-127.

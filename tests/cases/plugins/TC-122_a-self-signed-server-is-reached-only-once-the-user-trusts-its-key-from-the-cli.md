---
id: TC-122
title: A server with a self-signed certificate is reached over wss and https only once the user trusted its key with the CLI, and refused again when the key changes
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
- TC-120's preconditions, run as written (in S-12: kept from that stage): Acme relay installed, every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- The voice server with TLS on 48122, showing the made-up self-signed certificate A (`tests/fixtures/certs/voice-a.pem`; no system trusts it):

```bash
nohup python3 tests/fixtures/ws_echo.py 48122 "$D/wss.log" --tls tests/fixtures/certs/voice-a.pem tests/fixtures/certs/voice-a.key > "$D/wss.out" 2>&1 & echo $! > "$D/wss.pid"
```
- Its address listed with the plain one: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121, https://127.0.0.1:48122"`

## Test Data

| Field | Value |
|---|---|
| Key of certificate A | `sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=` |
| Key of certificate B | `sha256//FnAJ8/qwoiAdJV/I5uE+TYf9kpG5WbHcCqDlA/p972I=` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay rpc '{"url":"wss://127.0.0.1:48122/rpc"}'` | It fails with `the certificate at 127.0.0.1:48122 isn't one this computer trusts (its key: sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=): if it is your own server's, trust it in Settings → Plugins → Acme relay, or with \`echoctl plugin trust acme-relay 127.0.0.1:48122\`` |
| 2 | `echoctl plugin call acme-relay get '{"url":"https://127.0.0.1:48122/ping"}'`; then `cat "$D/wss.log" 2>/dev/null | grep -c "^ws\|^rpc\|^get"` | It fails the same way, naming the same key; `0`: no request went over |
| 3 | `echoctl plugin certs acme-relay` | `[{"address": "127.0.0.1:48122", "changed": false, "pinned": null, "seen": "sha256//Xy96…Flc="}]` (the full key of A) |
| 4 | `ECHOCTL_HEADLESS=1 echoctl plugin trust acme-relay 127.0.0.1:48122`; then `echoctl plugin certs acme-relay` | It fails with `plugins are installed, updated and changed by you, not from inside a run Echo started`; the list is as in step 3 |
| 5 | `echoctl plugin trust acme-relay 127.0.0.1:1` | It fails with `127.0.0.1:1 isn't an https address you listed for Acme relay: list it in its settings first` |
| 6 | `echoctl plugin trust acme-relay 127.0.0.1:48122`; then the calls of steps 1 and 2 again | `Trusted 127.0.0.1:48122 for acme-relay: sha256//Xy96…Flc=`; the `rpc` call prints the JSON-RPC answer (`"id": 7`, `"method": "speak"`); the `get` call prints `{"body": "{\"ok\": true}", "status": 200}` |
| 7 | Restart the server with certificate B on the same port: `kill $(cat "$D/wss.pid")`, then the precondition's `nohup` line with `voice-b.pem` / `voice-b.key`; then the `rpc` call of step 1 | It fails with `the certificate at 127.0.0.1:48122 changed since you trusted it (its key is now sha256//FnAJ8…972I=; you trusted sha256//Xy96…Flc=): refused until you trust it in Settings → Plugins → Acme relay, or with \`echoctl plugin trust acme-relay 127.0.0.1:48122\`` |
| 8 | `echoctl plugin certs acme-relay` | `pinned` is A's key, `seen` B's, `changed` true |
| 9 | `echoctl plugin trust acme-relay https://127.0.0.1:48122`; the `rpc` call again; then `echoctl plugin untrust acme-relay 127.0.0.1:48122`; the `rpc` call again | `Trusted … sha256//FnAJ8…972I=`; the answer comes; `No longer trusted at 127.0.0.1:48122 for acme-relay`; the call fails again with `isn't one this computer trusts` |

## Expected Final State
Acme relay trusts no certificate; the server on 48122 shows certificate B and has met it once more (kept as met).

## Teardown
- `kill $(cat "$D/wss.pid")`; `echoctl plugin untrust acme-relay 127.0.0.1:48122`. Kept otherwise for TC-123 in S-12 (it starts its own server). On its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-82). The key is the SHA-256 of the certificate's public key, as curl's `--pinnedpubkey` takes it: `openssl x509 -in tests/fixtures/certs/voice-a.pem -pubkey -noout | openssl pkey -pubin -outform der | openssl dgst -sha256 -binary | base64`.
- That a plugin can't write a pin through its store is proven by the Rust tests (`jshost::net`).

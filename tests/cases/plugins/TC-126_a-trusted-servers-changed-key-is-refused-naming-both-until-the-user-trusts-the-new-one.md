---
id: TC-126
title: A trusted server whose certificate's key changed is refused, naming both keys, until the user trusts the new one
priority: high
type: functional
requirements: [F-PLUG-82]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, tls, pinning, cli]
---

# TC-126: The user's voice server comes back with a new key, and Echo refuses it until they trust that one

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- TC-122's preconditions and its step 6 (in S-39: kept from that stage): Acme relay installed, the voice server with TLS on 48122 showing certificate A, its key trusted; every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`

## Test Data

| Field | Value |
|---|---|
| Key of certificate A (trusted) | `sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=` |
| Key of certificate B | `sha256//FnAJ8/qwoiAdJV/I5uE+TYf9kpG5WbHcCqDlA/p972I=` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Restart the server with certificate B on the same port: `kill $(cat "$D/wss.pid")`, then `nohup python3 tests/fixtures/ws_echo.py 48122 "$D/wss.log" --tls tests/fixtures/certs/voice-b.pem tests/fixtures/certs/voice-b.key > "$D/wss.out" 2>&1 & echo $! > "$D/wss.pid"`; then `echoctl plugin call acme-relay rpc '{"url":"wss://127.0.0.1:48122/rpc"}'` | It fails with `the certificate at 127.0.0.1:48122 changed since you trusted it (its key is now sha256//FnAJ8…972I=; you trusted sha256//Xy96…Flc=): refused until you trust it in Settings → Plugins → Acme relay, or with \`echoctl plugin trust acme-relay 127.0.0.1:48122\`` |
| 2 | `echoctl plugin certs acme-relay` | `pinned` is A's key, `seen` B's, `changed` true |
| 3 | `echoctl plugin trust acme-relay https://127.0.0.1:48122`; then the `rpc` call of step 1 again | `Trusted 127.0.0.1:48122 for acme-relay: sha256//FnAJ8/qwoiAdJV/I5uE+TYf9kpG5WbHcCqDlA/p972I=`; the call prints the JSON-RPC answer (`"id": 7`) |

## Expected Final State
Acme relay trusts certificate B's key at 127.0.0.1:48122; the server there shows certificate B.

## Teardown
- In S-39 kept for TC-127. On its own: `kill $(cat "$D/wss.pid")`; `echoctl plugin untrust acme-relay 127.0.0.1:48122`; TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-82): a pin holds the certificate's key, so a certificate renewed with the same key passes and a new key doesn't (the Rust test `jshost::net::tests::a_certificate_the_system_doesnt_trust_is_reached_only_once_the_user_trusts_its_key_and_refused_when_it_changes` checks https and a stream too).

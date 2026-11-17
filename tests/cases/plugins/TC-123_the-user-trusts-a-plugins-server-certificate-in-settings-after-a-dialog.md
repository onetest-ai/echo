---
id: TC-123
title: Settings → Plugins shows the certificate Echo met at a plugin's address; Trust takes it only after the user's yes, Forget drops it
priority: high
type: functional
requirements: [F-PLUG-82]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, tls, pinning, settings]
---

# TC-123: The user trusts their voice server's certificate in the plugin's settings

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- **The user's answer set to yes** (a development build answers the system's dialog from `ECHOCTL_ALLOW_PINS`; the dialog itself is for a person): data built right before the run with `ECHOCTL_ALLOW_PINS=yes tests/fixtures/serve.sh up <dir> 48120 --set small`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); the browser 1440 × 900. In S-12 the prelude starts the server so
- TC-120's preconditions (in S-12: kept from that stage), every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- The voice server with TLS on 48122 showing certificate A (TC-122's precondition line, `voice-a.*`), its address listed: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121, https://127.0.0.1:48122"`, and nothing trusted there (`echoctl plugin untrust acme-relay 127.0.0.1:48122`)
- Echo met its certificate once: `echoctl plugin call acme-relay rpc '{"url":"wss://127.0.0.1:48122/rpc"}'` fails naming `sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins` and reload it (the page reads the plugins again on a load); on the tab "Installed" click "Acme relay" | Its detail shows a heading "Certificates" with a group "Certificate of 127.0.0.1:48122": `127.0.0.1:48122`, the note "Its certificate isn't one this computer trusts: sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=", and the buttons "Trust" and "Dismiss" |
| 2 | Click "Trust"; wait up to 5 s | The group now says "Trusted: sha256//Xy96ZGBl7ULObmus5R49MQ/GWyx26THhqCJCxNfPFlc=" with the button "Forget" and no "Trust" |
| 3 | `echoctl plugin certs acme-relay`; then `echoctl plugin call acme-relay rpc '{"url":"wss://127.0.0.1:48122/rpc"}'` | `pinned` is that key, `seen` null; the call prints the JSON-RPC answer (`"id": 7`) |
| 4 | Click "Forget"; wait up to 5 s | The heading "Certificates" and its group are gone |
| 5 | `echoctl plugin certs acme-relay` | `[]` |

## Expected Final State
Acme relay trusts no certificate.

## Teardown
- `kill $(cat "$D/wss.pid")`. In S-12 kept for TC-124; on its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-82). The page can't be told apart from a plugin's own window half, so Trust asks in a dialog of the system's; "no" (nothing trusted, and the page can't ask again for a minute) and a key Echo didn't meet (refused at once) are proven by the Rust test `plugins::pins::the_window_trusts_only_the_key_echo_met_and_only_on_the_users_yes`.

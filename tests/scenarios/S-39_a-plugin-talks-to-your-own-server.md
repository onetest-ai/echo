---
id: S-39
title: A plugin talks to your own server: WebSocket and an event stream to the address you listed, a self-signed certificate trusted, changed and untrusted, big downloads and file uploads, at most 8 connections
run: agent
data: small
cases:
  - tests/cases/plugins/TC-120_a-plugins-host-half-talks-json-rpc-over-websocket-only-to-an-address-the-user-listed.md
  - tests/cases/plugins/TC-121_a-plugins-host-half-reads-an-event-stream-as-it-comes.md
  - tests/cases/plugins/TC-122_a-self-signed-server-is-reached-only-once-the-user-trusts-its-key-from-the-cli.md
  - tests/cases/plugins/TC-126_a-trusted-servers-changed-key-is-refused-naming-both-until-the-user-trusts-the-new-one.md
  - tests/cases/plugins/TC-127_a-certificate-the-user-untrusts-is-refused-again.md
  - tests/cases/plugins/TC-123_the-user-trusts-a-plugins-server-certificate-in-settings-after-a-dialog.md
  - tests/cases/plugins/TC-124_a-plugin-saves-a-download-over-10-mb-into-its-store.md
  - tests/cases/plugins/TC-128_a-file-a-plugin-sends-arrives-whole-at-the-path-asked-without-its-name.md
  - tests/cases/plugins/TC-129_a-plugin-sends-a-file-only-by-its-own-handle-with-post-put-or-patch-never-with-text.md
  - tests/cases/plugins/TC-125_a-plugin-keeps-at-most-8-connections-open.md
---

# S-39: A plugin talks to your own server

**Story:** Sam runs a voice server of their own on this computer and installs Acme relay, a plugin whose host half talks to it. It reaches nothing until Sam lists the server's address; then it talks JSON-RPC over a WebSocket and reads an event stream. The server's https has a self-signed certificate: refused until Sam trusts its key — from the terminal, then again when the key changes, and from Settings after a dialog — and refused again once Sam takes the trust back. It saves a big download into its store and sends a file Sam handed it to the path it asked, and it never keeps more than 8 connections open.

**Data:** `small`, served with the user's answer to the "trust this certificate" dialog set to yes (a development build reads it from `ECHOCTL_ALLOW_PINS`): `ECHOCTL_ALLOW_PINS=yes tests/fixtures/serve.sh up <dir> 48120 --set small`. Every Bash call starts with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`.

**Length:** about 10 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build, served on 48120 with `ECHOCTL_ALLOW_PINS=yes` | the lead (`serve.sh up`) | 1–10 (the window in 6) |
| Acme relay (`acme-net`) and the voice server on 48121 | stage 1 (its preconditions) | 1–10 (removed by stage 10's teardown) |
| The voice server with TLS on 48122 (certificate A, then B) | stage 3 (its precondition lines) | 3–5; stage 6 starts its own |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-120_a-plugins-host-half-talks-json-rpc-over-websocket-only-to-an-address-the-user-listed.md` | — | Its marketplace is its own `acme-net`: run its precondition lines as written, except `serve.sh up` (the lead's). Skip its teardown |
| 2 | `plugins/TC-121_a-plugins-host-half-reads-an-event-stream-as-it-comes.md` | 1 | Acme relay is installed with 48121 listed: skip the precondition lines. Skip its teardown |
| 3 | `plugins/TC-122_a-self-signed-server-is-reached-only-once-the-user-trusts-its-key-from-the-cli.md` | 1 | Run only the precondition lines that start the TLS server (certificate A) and list the addresses. Skip its teardown |
| 4 | `plugins/TC-126_a-trusted-servers-changed-key-is-refused-naming-both-until-the-user-trusts-the-new-one.md` | 3 | Certificate A trusted by stage 3: skip the precondition lines. Skip its teardown |
| 5 | `plugins/TC-127_a-certificate-the-user-untrusts-is-refused-again.md` | 4 | Certificate B shown and trusted by stage 4: skip the precondition lines. Its teardown stops the TLS server |
| 6 | `plugins/TC-123_the-user-trusts-a-plugins-server-certificate-in-settings-after-a-dialog.md` | 1 | Served with `ECHOCTL_ALLOW_PINS=yes` by the lead: run only the precondition lines that start the TLS server with certificate A, list the addresses, untrust and meet the certificate |
| 7 | `plugins/TC-124_a-plugin-saves-a-download-over-10-mb-into-its-store.md` | 1 | Run only the precondition lines that make the file server and list the addresses |
| 8 | `plugins/TC-128_a-file-a-plugin-sends-arrives-whole-at-the-path-asked-without-its-name.md` | 1 | Skip the TC-120 line; run the others |
| 9 | `plugins/TC-129_a-plugin-sends-a-file-only-by-its-own-handle-with-post-put-or-patch-never-with-text.md` | 1 | Skip the TC-120 line; run the others |
| 10 | `plugins/TC-125_a-plugin-keeps-at-most-8-connections-open.md` | 1 | Skip the TC-120 line; run the address line. Its teardown removes Acme relay, `acme-net` and the voice server, but not the lead's window server (`serve.sh down` is the lead's at the end) |

## Notes
- Every server here is made up and on this computer (`tests/fixtures/ws_echo.py`, `python3 -m http.server`), on ports 48121–48124; the certificates are made-up self-signed ones in `tests/fixtures/certs/`. Nothing is fetched from the network.
- Only stage 6 uses the window (Settings → Plugins); the others are the CLI. The host half runs in the terminal's own process (`echoctl plugin call`) as it does in the app.
- What a case can't show is proven by the Rust tests (`jshost::net`, `plugins::pins`): a binary message and a file as one, the message caps (16 MB out, 4 MB in: a bigger one closes the connection, code 1009), what waits for a plugin bounded (256 events or 16 MB read ahead, 8 MB waiting for handlers), a stream cancelled, a plugin's connections closed when it is switched off, and `trust` without `--key` refused once Echo met more than one key at an address.

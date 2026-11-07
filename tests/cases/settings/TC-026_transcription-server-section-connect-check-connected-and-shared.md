---
id: TC-026
title: Settings → Transcription server — the OpenAI-compatible form's Connect explains what's wrong, a connected server and sharing show in the section
priority: medium
type: functional
requirements: [F-SET-10]
module: settings
run: agent
platform: macos
data: small
tags: [settings, transcription, server]
---

# TC-026: Settings → Transcription server — the OpenAI-compatible form's Connect explains what's wrong, a connected server and sharing show in the section

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set built right before the run: `tests/fixtures/serve.sh up <dir> <port>`; the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- `{{stt_server_url}}` and `{{stt_server_key}}`: a second computer on the network runs a dev Echo on test data with `echoctl server share on` (its printed URL and key). Without one, steps 4–5 are BLOCKED, not failed
- Ollama runs on this computer (`http://127.0.0.1:11434`) with no speech-to-text model
- The section is the `h3` "Transcription server" and its parent; read it with `innerText`. Never print the API key in the run's output: compare it masked (first 4, last 2)

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#settings`, click "Transcription" | The URL ends `#settings/transcription`; the section "Transcription server" has "Use a server" with "Echo server…" and "OpenAI-compatible…", and "Transcribe for other computers" with its switch off (`aria-checked="false"`) and "Share this computer's model on your network." |
| 2 | Click "OpenAI-compatible…" | A form "OpenAI-compatible server" opens with the address prefilled `http://localhost:11434`, a model field ("e.g. lfm3-audio, whisper-1"), a key field "(optional)" and "Connect" |
| 3 | Set the address to `http://127.0.0.1:9`, click "Connect"; then set it to `http://127.0.0.1:11434` and click "Connect" | First "could not reach http://127.0.0.1:9: … Connection refused"; then "the server lists no speech-to-text model (…)" with the models it has. Nothing is kept: `echoctl server status` still says "transcribing on this computer" |
| 4 | Run `echoctl server use {{stt_server_url}} --key {{stt_server_key}}`, reload the page | The CLI prints "transcribing on <the other computer's name> (<model>, …)"; the section reads "Connected to <that name> ({{stt_server_url}}); this computer's model takes over when it's down." with a "Disconnect" button and no "Echo server…" |
| 5 | Run `echoctl server off`, then `echoctl server share on`, reload the page | The section is back to "Echo server…" / "OpenAI-compatible…"; "Transcribe for other computers" reads "Shared at `http://<this Mac's address>:47800` — key `<key>`" with the switch on; the key is the one `echoctl server status` prints (masked compare) |

## Expected Final State
The section shows what the config says, and Connect on the form checks the server before keeping anything.

## Teardown
- `echoctl server share off`; `echoctl server off`; `tests/fixtures/serve.sh down <dir>`; leave one blank tab

## Notes
- "Echo server…", "Disconnect" and the share switch are app actions (native prompts, a listening port): a served window can't do them, so steps 4–5 set the state from the CLI and check how the window shows it. Clicking them in the real window stays person-run (`F-SET-10`, the app half).
- `share on` from the CLI only writes the test config: the port opens when an app runs on that config, and none does here.
- The no-speech-model message says "name one with --model", the CLI flag, though the form has its own model field (wording, #143).
- Run on 2026-11-07 on the dev Mac, the second computer a Mac mini on the LAN: PASS.

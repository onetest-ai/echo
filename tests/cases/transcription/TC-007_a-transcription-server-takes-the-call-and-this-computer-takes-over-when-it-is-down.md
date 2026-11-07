---
id: TC-007
title: A transcription server (another Echo) transcribes a call; when it is down this computer's model takes over, and the transcript says which
priority: medium
type: functional
module: transcription
requirements: [F-STT-05]
tags: [transcription, server, network, fallback]
run: agent
platform: macos
data: small
---

# TC-007: A transcription server takes the call; this computer takes over when it is down

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set built right before the run (`tests/fixtures/make_data.py <dir> --set small`); every command runs as `target/debug/echoctl …` after `. <dir>/env`
- **A transcription server:** another Echo with `echoctl server share on` (a second computer on the network, or a second dev app on this one with its own `ECHOCTL_CONFIG_DIR`/`ECHOCTL_HOME`), with a speech model; its URL and API key from `share on`. Whoever runs it turns sharing off when step 5 asks
- A speech model in the run's own config (`transcription/TC-001`: linked from an installed Echo's models or downloaded), active
- The known conversations are built (`tests/fixtures/audio/make_audio.py`, no network needed)

## Test Data

| Call | Built from | Transcribed on |
|---|---|---|
| `<day>_120000-acme-sync-stt` | `acme-sync` (36 s, 6 turns) | the server |
| `<day>_123000-acme-sync-fallback` | `acme-sync` | this computer (the server is down) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `echoctl server use <URL> --key <KEY>` | It prints "transcribing on <server name> (<model>, whisper.cpp, …)" (the key is checked before it is kept) |
| 2 | `echoctl server status` | "transcribing on <URL> — <server name> (…)" |
| 3 | `make_audio.py build acme-sync $ECHOCTL_HOME/calls/<day>_120000-acme-sync-stt`; `echoctl transcribe <that folder>` | Exit 0, "done in …s"; the audio is deleted |
| 4 | `make_audio.py score <that folder>`; `head -6 <that folder>/transcript.md` | `pass` true, `wer` ≤ 0.15, `transcribed_in_silence` []; the header reads "**Model:** large-v3-turbo-q5_0 on <server name>" |
| 5 | The server's owner runs `echoctl server share off`; wait until `curl -s -o /dev/null -w '%{http_code}' <URL>/` gets no connection | (`000`) |
| 6 | Build `acme-sync` into `<day>_123000-acme-sync-fallback`; `echoctl transcribe <that folder>` | Its output has "transcription server: could not reach <URL>: … Connection refused" and "model: large-v3-turbo-q5_0 (server not reachable)"; exit 0 |
| 7 | `make_audio.py score <that folder>`; `head -6 <that folder>/transcript.md` | `pass` true, `wer` ≤ 0.15; the header reads "**Model:** large-v3-turbo-q5_0 (server not reachable)"; no `transcribe.error` |

## Expected Final State
Both calls are transcribed, one on the server and one here; the run's config still names the server (`server status`), so the next call tries it first again.

## Teardown
- `echoctl server off` (the run's own config); the server's owner stops its app

## Notes
- Verified 2026-11-07 with a second Mac as the server: on the server 9 s for 36 s of audio, WER 0.0; the fallback 5 s, WER 0.0.
- The OpenAI-compatible kind (`server use --openai --model …`) and the Settings UI are `settings/TC-026`.

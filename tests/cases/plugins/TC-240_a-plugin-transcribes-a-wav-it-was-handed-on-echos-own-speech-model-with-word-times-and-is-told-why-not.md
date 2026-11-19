---
id: TC-240
title: A plugin that asked for speech transcribes a WAV it was handed on Echo's own speech model — text, language, segments and, when asked, word times — and is told why not for a file it wasn't handed, a file that isn't a WAV, a language Echo doesn't know, or while Echo transcribes a call
priority: high
type: functional
requirements: [F-PLUG-150, F-PLUG-151]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, speech, transcription, cli]
---

# TC-240: A plugin transcribes a WAV it was handed on Echo's own speech model

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> 48230 --set small`; every Bash call starts with the prelude (a fresh shell each time): `set -a; . <dir>/env; set +a; E=target/debug/echoctl` (built right before); abort if `$ECHOCTL_HOME` or `$ECHOCTL_CONFIG_DIR` isn't inside `<dir>`
- **A speech model in the run's own config**, English (`transcription/TC-001`'s way: linked from an installed Echo's models, or downloaded: `$E model download small-q8_0`, 264 MB), active; the test never runs without one
- The known conversation's audio, built without a network: `python3 tests/fixtures/audio/make_audio.py build very-short "$(dirname "$ECHOCTL_HOME")/vs"` — `vs/remote.wav` holds "Quick one: is the call at three still on?" (9.3 s of audio, the speech from 1.0 s to 4.5 s)
- A copy of the test marketplace, and Speech demo installed from it: `C=$(mktemp -d)/m; cp -R tests/fixtures/plugins "$C"; $E plugin marketplace add "$C" --name test-plugins; $E plugin install speech-demo --from test-plugins --yes`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin); a call folder made by the case, removed by its teardown

## Test Data

| Field | Value |
|---|---|
| Plugin | Speech demo (`speech-demo`) 0.1.0: `uses: {"speech": true}`; its command `transcribe` answers `ctx.speech.transcribe(<the file it was handed>, <its JSON arguments>)` |
| Audio | `vs/remote.wav` (16 kHz mono WAV); a text file renamed `talk.mp3` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | The install's sheet (the precondition's output) | It lists "Transcribes audio files you hand it (WAV, up to 60 minutes and 200 MB) with Echo's own speech model, on this computer, or on the transcription server you connected; it starts only while Echo is not recording or transcribing a call. The text goes only to the plugin" |
| 2 | `$E plugin call speech-demo transcribe '{"words":true}' --file "$(dirname "$ECHOCTL_HOME")/vs/remote.wav"` | Exit 0, JSON: `text` contains "three" and "call" (case aside), `language` `en`; `segments` is not empty, each `{start, end, text}` with `start` < `end` and every time within 0 and 9.3 s, in order; `words` has one entry `{start, end, word}` for each word of `text` (the same words, in order), `start` ≤ `end`, in order, the first not before 0.5 s |
| 3 | The same without `words`: `… '{}' --file …` | Exit 0; the same `text` and `segments`; no `words` key |
| 4 | `… '{"language":"uk"}' --file …` | Exit 0 (whatever it hears), `language` `uk`: the language asked, not Settings' |
| 5 | `… '{"language":"xx-nope"}' --file …` | Exit 1: `Error: “xx-nope” isn't a language code Echo transcribes in` |
| 6 | `$E plugin call speech-demo transcribe '{}'` (no `--file`) | Exit 1: an error that names no file handed to speech-demo — nothing is decoded |
| 7 | `printf 'ID3 not audio' > "$(dirname "$ECHOCTL_HOME")/talk.mp3"`; `… '{}' --file "$(dirname "$ECHOCTL_HOME")/talk.mp3"` | Exit 1: `Error: ctx.speech.transcribe reads WAV files only` |
| 8 | A call being transcribed: make a call folder with a `meta.json` (`mkdir "$ECHOCTL_HOME/calls/2026-11-17_100000_acme"; echo '{}' > "$ECHOCTL_HOME/calls/2026-11-17_100000_acme/meta.json"`), and a live process's id in its marker: `sleep 300 & echo $! > "$ECHOCTL_HOME/calls/2026-11-17_100000_acme/.transcribing"`; step 2's call again | Exit 1: `Error: Echo is recording or transcribing a call — a plugin's audio is transcribed once it is done`; Echo does not wait, the plugin asks again |
| 9 | `kill` the `sleep`; step 2's call again | Exit 0: the same text as step 2 (a marker whose process is gone holds nothing back) |

## Expected Final State
Speech demo is installed; nothing else of the run's data changed but the call folder step 8 made.

## Teardown
- `kill` the `sleep` if it still runs; `rm -rf "$ECHOCTL_HOME/calls/2026-11-17_100000_acme"`; `$E plugin remove speech-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from the feature's text (F-PLUG-150, F-PLUG-151, #834), not from the code. The host half runs here in the terminal's own process (`echoctl plugin call`), as it does in the app.
- The caps (an hour of audio from the header, 200 MB), a file handed to another plugin, a plugin that didn't ask for speech, and a recording in progress are proven by the Rust tests `jshost::speech::tests`: a case cannot make an hour of audio cheaply or start a recording without the call app.
- Both engines time their words: Whisper (token times) and Parakeet / Nemotron; a model that gave none leaves `words` out (`jshost::speech::tests::words_come_only_when_asked_and_when_the_engine_timed_every_sentence`).

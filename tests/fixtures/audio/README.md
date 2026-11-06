# Known conversations

Short made-up calls whose words are known, for transcription tests now and recording tests later. `conversations.json`
holds what is said: each turn's speaker (`me` → the mic track, anyone else → the remote track), voice, language, text
and the silence before it. `clips/` holds each turn as a small mp3 (16 kHz mono, 32 kbit/s), named by a hash of voice,
language and text, committed so nothing needs a key or the network to build a call.

| Command | What |
|---|---|
| `make_audio.py list` | the conversations |
| `make_audio.py build <id> <call dir>` | a call folder: `mic.wav`, `remote.wav`, `meta.json`, `expected.json` |
| `make_audio.py score <call dir>` | the transcript against `expected.json`: word errors per turn and overall (default max 15%), speaker and language per segment, nothing in silence; exit 1 on a failure |
| `make_audio.py render --secrets <file>` | synthesise turns not in `clips/` yet: English by Azure OpenAI (`gpt-4o-mini-tts`), Ukrainian and Russian by ElevenLabs (`eleven_multilingual_v2`); keys from the environment or the `env` of the given JSON file, never written |

Adding a conversation: add it to `conversations.json`, run `render`, commit the new clips. Ukrainian lines carry at
least one Ukrainian-only letter (і ї є ґ), or the voice reads them with Russian vowels. Keep calls short (under a
minute): the clips are committed.

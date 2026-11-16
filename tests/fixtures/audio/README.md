# Known conversations

Short made-up calls whose words are known, for transcription tests now and recording tests later. `conversations.json`
holds what is said: each turn's speaker (`me` → the mic track, anyone else → the remote track), voice, language, text
and the silence before it. `clips/` holds each turn as a small mp3 (16 kHz mono, 32 kbit/s), named by a hash of voice,
language and text, committed so nothing needs a key or the network to build a call.

| Command | What |
|---|---|
| `make_audio.py list` | the conversations |
| `make_audio.py build <id> <call dir> [--languages en,uk,ru]` | a call folder: `mic.wav`, `remote.wav`, `meta.json` (its `languages`: the given ones, else the turns'), `expected.json` |
| `make_audio.py score <call dir>` | the transcript against `expected.json`: word errors per turn and overall (default max 15%), speaker and language per segment, nothing in silence; exit 1 on a failure |
| `make_audio.py render --secrets <file>` | synthesise turns not in `clips/` yet: English by Azure OpenAI (`gpt-4o-mini-tts`), Ukrainian and Russian by ElevenLabs (`eleven_multilingual_v2`); keys from the environment or the `env` of the given JSON file, never written |

Adding a conversation: add it to `conversations.json`, run `render`, commit the new clips. Ukrainian lines carry at
least one Ukrainian-only letter (і ї є ґ), or the voice reads them with Russian vowels. Keep calls short (under a
minute): the clips are committed. `minutes-call` (213 s) is the one exception: it reuses acme-sync's and two-remotes'
turns three times over (no new clips) so a call passes the minutes gate (3 minutes, 1,500 characters).
`speakers-echo` (126 s) reuses acme-sync's and two-remotes' turns twice: its `echo` (`{"gain": 0.35, "delay_ms": 40}`)
makes `build` add the remote track to the mic that much quieter and later, as laptop speakers do, and four of the
user's lines have a `gap` below 0, starting before the other side's line ends (talking over them).
`ukrainian-call` and `russian-call` reuse `mixed-languages`' Ukrainian and Russian lines (no new clips): a call has one
language, and each must come out in its own with en, uk and ru allowed.

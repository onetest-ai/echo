---
id: S-14
title: Transcription quality on known conversations: words, speakers, languages, silence
run: agent
data: small
cases:
  - tests/cases/transcription/TC-001_content-and-speakers.md
  - tests/cases/transcription/TC-003_languages-restricted.md
  - tests/cases/transcription/TC-002_two-languages.md
  - tests/cases/transcription/TC-005_silence.md
  - tests/cases/transcription/TC-007_a-transcription-server-takes-the-call-and-this-computer-takes-over-when-it-is-down.md
  - tests/cases/settings/TC-026_transcription-server-section-connect-check-connected-and-shared.md
---

# S-14: Transcription quality on known conversations: words, speakers, languages, silence

**Story:** Four made-up calls whose words are known (`tests/fixtures/audio/`: English through Azure, Ukrainian and
Russian through ElevenLabs) are turned into call folders, transcribed on this computer by the build under test, and
scored against what was said: word errors, who spoke, the language of each sentence, and nothing out of silence.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `transcription/TC-001_content-and-speakers.md` | — | Link the speech model into the test config and `model use` it (the cases' Preconditions) |
| 2 | `transcription/TC-003_languages-restricted.md` | — | — |
| 3 | `transcription/TC-002_two-languages.md` | — | — |
| 4 | `transcription/TC-005_silence.md` | — | — |
| 5 | `transcription/TC-007_a-transcription-server-takes-the-call-and-this-computer-takes-over-when-it-is-down.md` | — | A second computer shares its model (`echoctl server share on` on its own test data): `{{stt_server_url}}`, `{{stt_server_key}}`; without one, BLOCKED |
| 6 | `settings/TC-026_transcription-server-section-connect-check-connected-and-shared.md` | — | The same second computer; the window served on small data (`serve.sh up`) |

## Notes
- CLI only but stage 6 (a served window). Each stage builds its own call folder, so the stages don't depend on each other.
- Needs the speech model on this machine (`~/Library/Application Support/echo/models`); about 10 s a call on Apple Silicon.

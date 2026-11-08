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

**Story:** Made-up calls whose words are known (`tests/fixtures/audio/`: English through Azure, Ukrainian and
Russian through ElevenLabs), played through TestCall, captured and transcribed on this computer by the build under test
**once** (`tests/fixtures/recordings.py prepare`, before the run), are scored against what was said: word errors, who
spoke, the language of each sentence, and nothing out of silence. This is the one place their words are scored; then
the transcription server and this computer taking over, on one short clip per engine.

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded
calls"; outside any sandbox). Stages 1–4 copy its calls and transcribe nothing.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `transcription/TC-001_content-and-speakers.md` | — | — |
| 2 | `transcription/TC-003_languages-restricted.md` | — | — |
| 3 | `transcription/TC-002_two-languages.md` | — | — |
| 4 | `transcription/TC-005_silence.md` | — | — |
| 5 | `transcription/TC-007_a-transcription-server-takes-the-call-and-this-computer-takes-over-when-it-is-down.md` | — | A second computer shares its model (`echoctl server share on` on its own test data): `{{stt_server_url}}`, `{{stt_server_key}}`; without one, BLOCKED |
| 6 | `settings/TC-026_transcription-server-section-connect-check-connected-and-shared.md` | — | The same second computer; the window served on small data (`serve.sh up`) |

## Notes
- CLI only but stage 6 (a served window). Stages 1–4 each copy their own prepared call, so they don't depend on each other.
- Transcriptions in this scenario: two (stage 5: the server and the fallback, 9 s each). Before 2026-11-08: six (each
  stage built its call from the clips and transcribed it, the same conversations S-17 and S-18 then captured and
  transcribed again).
- Stage 5 needs the speech model in the run's config (`~/Library/Application Support/echo/models`, linked).

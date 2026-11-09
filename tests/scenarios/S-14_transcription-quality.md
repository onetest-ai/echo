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
  - tests/cases/transcription/TC-008_turbo-q8-detects-its-own-language.md
---

# S-14: Transcription quality on known conversations: words, speakers, languages, silence

**Story:** Made-up calls whose words are known (`tests/fixtures/audio/`: English through Azure, Ukrainian and
Russian through ElevenLabs), fed from files to `echoctl capture` (no device, no sound), captured and transcribed on this computer by the build under test
**once** (`tests/fixtures/recordings.py prepare`, before the run), are scored against what was said: word errors, who
spoke, the language of each sentence, and nothing out of silence. This is the one place their words are scored; then
the transcription server and this computer taking over, on one short clip per engine.

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded
calls"; about 1.5 min, silent, no device or permission). Stages 1–4 copy its calls and transcribe nothing.

**Data:** `small`: `tests/fixtures/make_data.py <dir>` (stages 1–5 and 7, CLI); stage 6 serves the window (`serve.sh up`).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small`) | the lead | all |
| Prepared recordings of this build (`acme-sync`, `two-remotes`, `mixed-languages`, `long-silence`: their transcripts and `transcribe.out`) — no live call in this scenario | the lead (`recordings.py prepare`, once per build) | 1, 2, 3, 4 |
| A transcription server on a second computer (`echoctl server share on` on its own test data): `{{stt_server_url}}`, `{{stt_server_key}}` | the second computer's owner | 5, 6 |
| Two transcriptions of `very-short` (9 s): one on the server, one on this computer when it is down | stage 5 | 5 |
| A transcription of `mixed-languages` (31 s) on turbo q8_0 | stage 7 | 7 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `transcription/TC-001_content-and-speakers.md` | — | — |
| 2 | `transcription/TC-003_languages-restricted.md` | — | — |
| 3 | `transcription/TC-002_two-languages.md` | — | — |
| 4 | `transcription/TC-005_silence.md` | — | — |
| 5 | `transcription/TC-007_a-transcription-server-takes-the-call-and-this-computer-takes-over-when-it-is-down.md` | — | A second computer shares its model (`echoctl server share on` on its own test data): `{{stt_server_url}}`, `{{stt_server_key}}`; without one, BLOCKED |
| 6 | `settings/TC-026_transcription-server-section-connect-check-connected-and-shared.md` | — | The same second computer; the window served on small data (`serve.sh up`) |
| 7 | `transcription/TC-008_turbo-q8-detects-its-own-language.md` | — | Both turbo models in the run's config (linked, as stage 5) |

## Notes
- CLI only but stage 6 (a served window). Stages 1–4 each copy their own prepared call, so they don't depend on each other. No live call: `prepare` captured the conversations from files, side by side, before the run.
- Transcriptions in this scenario: three (stage 5: the server and the fallback, 9 s each; stage 7: `mixed-languages` on
  q8_0, 31 s). Before 2026-11-08: six (each stage built its call from the clips and transcribed it, the same
  conversations S-17 and S-18 then captured and transcribed again).
- Stage 5 needs the speech model in the run's config (`~/Library/Application Support/echo/models`, linked: `recording/TC-001` step 1).
- Run 2026-11-08 evening (stages 1–4, on `prepare` of build 84b1bec, file-fed): PASS — `acme-sync` WER 0.0, every turn's speaker and language right; `two-remotes` `(languages: en)`, WER 0.029, only `en`; `mixed-languages` `(languages: uk, ru, en)`, WER 0.111, all five turns `lang_ok`; `long-silence` WER 0.0, nothing in the silence. 0.14–0.21 s per stage. Stages 5–6 not run (no second computer).

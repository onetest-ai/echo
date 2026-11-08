---
id: S-17
title: A recorded call end to end: record, take notes, transcript, automatic minutes, the files
run: agent
platform: macos
data: small
ai: fake
clock: fast
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the recordings of this build prepared (`tests/fixtures/recordings.py prepare`, once per run); the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens
cases:
  - tests/cases/recording/TC-014_test-call-two-tracks.md
  - tests/cases/recording/TC-001_record-next-call.md
  - tests/cases/window/TC-014_notes-while-recording.md
  - tests/cases/privacy/TC-003_no-audio-kept.md
  - tests/cases/window/TC-013_live-update-while-recording.md
  - tests/cases/transcription/TC-004_pin-language.md
  - tests/cases/recording/TC-013_very-short-call.md
  - tests/cases/claude/TC-003_auto-minutes.md
  - tests/cases/claude/TC-010_notes-move-minutes.md
  - tests/cases/window/TC-012_minutes-open-in-browser.md
  - tests/cases/recording/TC-019_tray-icon-follows-the-recording.md
---

# S-17: A recorded call end to end: record, take notes, transcript, automatic minutes, the files

**Environment:** this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens. How the run drives the test call: app_profile.md → "Recording with the test call".

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded calls"; outside any sandbox: inside one the audio is silent). It records and transcribes the known conversations once for this build and keeps them; the stages here copy them and transcribe nothing themselves.

**Data:** `small` on the fake and the test clock: `tests/fixtures/serve.sh up <dir> <port> --ai fake --clock fast` (hermetic: every Claude Code run is refused at once; automatic minutes 90 s after a call or the last note instead of 15 minutes), the speech model linked into its config (`recording/TC-001`'s Preconditions, for the watcher's own transcription), the window served with `ECHOCTL_TEST_OPEN_LOG_ONLY=1` (`serve.sh` passes the env file's variables; start the server with that variable too, or restart it as `window/TC-012` says).

**Story:** Sam records a call (two tracks), arms the next one and takes notes while it runs, watches a new call land in the window, sees that no audio stays, pins a language for one call, gets a transcript but no automatic minutes for a very short call, and sees the minutes start by themselves at the time Echo named — later when he adds a note; the minutes open in the browser and in Finder. What a model writes in them is S-19's.

**Length:** about 15 minutes: three live TestCall calls (36 + 36 + 9 s) and two automatic-minutes waits of 90 s (and a note's 90 s more).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-014_test-call-two-tracks.md` | — | — |
| 2 | `recording/TC-001_record-next-call.md` | — | — |
| 3 | `window/TC-014_notes-while-recording.md` | — | Load `{{base_url}}/` (the cookie) |
| 4 | `privacy/TC-003_no-audio-kept.md` | 2 | — |
| 5 | `window/TC-013_live-update-while-recording.md` | — | — |
| 6 | `transcription/TC-004_pin-language.md` | — | — |
| 7 | `recording/TC-013_very-short-call.md` | — | — |
| 8 | `claude/TC-003_auto-minutes.md` | — | — |
| 9 | `claude/TC-010_notes-move-minutes.md` | — | Same app as stage 8: stage 8's teardown (`app quit`) waits until this stage's end |
| 10 | `window/TC-012_minutes-open-in-browser.md` | — | — |
| 11 | `recording/TC-019_tray-icon-follows-the-recording.md` | — | **person** — skipped by an agent run (the tray can't be driven while the user's Echo runs) |

## Notes
- Live TestCall calls only where capturing is the subject: stages 2–3 (the watcher: arm, record, the call's clock for notes) and 5 (a call appearing while it records). Stage 1 reads the capture `prepare` made; stages 4, 6, 7, 8 and 9 copy prepared calls (`recordings.py copy`). Transcription quality is scored once, in S-14.
- Stages 2–3 and 8–9 start the second app instance and quit it at their end.
- What a model writes (the user's notes in the minutes, an automatic run's content and log) moved to S-19 on 2026-11-08 (`claude/TC-009`, `claude/TC-007`), with the local model.
- Only one TestCall call at a time on a Mac (port 7792, BlackHole): when other runs share the Mac, their watchers see this run's calls (unarmed: nothing recorded) — never arm two runs at once.
- Run 2026-11-07 on the dev Mac (the old plan: every stage its own call, the local model, the real 15 minutes): stages 1–11 PASS but for #141 and the mic fallback (#140). 2026-11-08 morning, the old plan: about 3 hours.

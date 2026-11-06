---
id: S-17
title: A recorded call end to end: record, take notes, transcript, minutes, the files
run: person
data: small
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1) and BlackHole 2ch; the watcher cases start a second app instance on the run's own data folder and config (never `~/echo`); person-run stages use the tray, Teams and macOS dialogs
cases:
  - tests/cases/recording/TC-014_test-call-two-tracks.md
  - tests/cases/recording/TC-001_record-next-call.md
  - tests/cases/window/TC-014_notes-while-recording.md
  - tests/cases/window/TC-013_live-update-while-recording.md
  - tests/cases/privacy/TC-003_no-audio-kept.md
  - tests/cases/transcription/TC-004_pin-language.md
  - tests/cases/recording/TC-013_very-short-call.md
  - tests/cases/recording/TC-019_tray-icon-follows-the-recording.md
  - tests/cases/claude/TC-009_own-notes-in-minutes.md
  - tests/cases/claude/TC-003_auto-minutes.md
  - tests/cases/claude/TC-010_notes-move-minutes.md
  - tests/cases/window/TC-012_minutes-open-in-browser.md
---

# S-17: A recorded call end to end: record, take notes, transcript, minutes, the files

**Environment:** this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1) and BlackHole 2ch; the watcher cases start a second app instance on the run's own data folder and config (never `~/echo`); person-run stages use the tray, Teams and macOS dialogs

**Story:** Sam records a call, takes notes while it runs, watches it land in the window, and gets minutes that keep the notes; a very short call gets a transcript only. Stages 1 to 7 run with the test call (agent); stages 8 to 12 need a person (tray, Claude Code runs).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-014_test-call-two-tracks.md` | — | — |
| 2 | `recording/TC-001_record-next-call.md` | — | — |
| 3 | `window/TC-014_notes-while-recording.md` | — | — |
| 4 | `window/TC-013_live-update-while-recording.md` | — | — |
| 5 | `privacy/TC-003_no-audio-kept.md` | — | — |
| 6 | `transcription/TC-004_pin-language.md` | — | — |
| 7 | `recording/TC-013_very-short-call.md` | — | — |
| 8 | `recording/TC-019_tray-icon-follows-the-recording.md` | — | — |
| 9 | `claude/TC-009_own-notes-in-minutes.md` | 3 | — |
| 10 | `claude/TC-003_auto-minutes.md` | 2 | — |
| 11 | `claude/TC-010_notes-move-minutes.md` | — | — |
| 12 | `window/TC-012_minutes-open-in-browser.md` | 10 | — |

## Notes
- Agent stages first. Stages 2 and 3 each start the second app instance and quit it at their end; stage 4 and 7 need the window served on the run's data.
- Minutes stages run Claude Code on the call's AI profile (Ollama qwen3.8 27B for tests).

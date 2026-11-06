---
id: S-17
title: A recorded call end to end: record, take notes, transcript, minutes, the files
run: person
env: this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists
cases:
  - tests/cases/recording/TC-001_record-next-call.md
  - tests/cases/window/TC-014_notes-while-recording.md
  - tests/cases/window/TC-013_live-update-while-recording.md
  - tests/cases/privacy/TC-003_no-audio-kept.md
  - tests/cases/claude/TC-009_own-notes-in-minutes.md
  - tests/cases/claude/TC-003_auto-minutes.md
  - tests/cases/claude/TC-010_notes-move-minutes.md
  - tests/cases/window/TC-012_minutes-open-in-browser.md
  - tests/cases/transcription/TC-004_pin-language.md
  - tests/cases/recording/TC-013_very-short-call.md
---

# S-17: A recorded call end to end: record, take notes, transcript, minutes, the files

**Environment:** this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists

**Story:** Sam records the next call, takes notes while it runs, watches it land in the window, and gets minutes that keep the notes; a very short call gets a transcript only.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-001_record-next-call.md` | — | — |
| 2 | `window/TC-014_notes-while-recording.md` | 1 | — |
| 3 | `window/TC-013_live-update-while-recording.md` | 1 | — |
| 4 | `privacy/TC-003_no-audio-kept.md` | 1 | — |
| 5 | `claude/TC-009_own-notes-in-minutes.md` | 2 | — |
| 6 | `claude/TC-003_auto-minutes.md` | 1 | — |
| 7 | `claude/TC-010_notes-move-minutes.md` | — | — |
| 8 | `window/TC-012_minutes-open-in-browser.md` | 6 | — |
| 9 | `transcription/TC-004_pin-language.md` | — | — |
| 10 | `recording/TC-013_very-short-call.md` | — | — |

## Notes
- Minutes stages run Claude Code on the call's AI profile (Ollama qwen3.8 27B for tests).

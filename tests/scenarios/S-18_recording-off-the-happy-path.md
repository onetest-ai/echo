---
id: S-18
title: Recording off the happy path: mid-call starts, cancels, drops, devices, mute, sleep, crashes
run: person
env: this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists
cases:
  - tests/cases/recording/TC-002_record-call-in-progress.md
  - tests/cases/recording/TC-003_cancel-arm.md
  - tests/cases/recording/TC-004_arm-expires.md
  - tests/cases/recording/TC-005_stop-by-hand.md
  - tests/cases/recording/TC-006_drop-and-rejoin.md
  - tests/cases/recording/TC-007_switch-audio-device.md
  - tests/cases/recording/TC-008_speakers-echo.md
  - tests/cases/permissions/TC-006_accessibility.md
  - tests/cases/recording/TC-009_mute-in-teams.md
  - tests/cases/recording/TC-010_sleep-while-recording.md
  - tests/cases/recording/TC-011_crash-while-recording.md
  - tests/cases/recording/TC-012_ask-mode.md
  - tests/cases/privacy/TC-001_no-recording-without-opt-in.md
  - tests/cases/privacy/TC-002_only-teams-recorded.md
  - tests/cases/cli/TC-004_arm-stop-terminal.md
  - tests/cases/transcription/TC-006_long-call.md
---

# S-18: Recording off the happy path: mid-call starts, cancels, drops, devices, mute, sleep, crashes

**Environment:** this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists

**Story:** Everything that can happen to a recording: starting it mid-call, cancelling, an arm that expires, stopping by hand, a drop and rejoin, switching to headphones and back, speakers, mute, the lid closed, a crash, ask mode, nothing recorded without opting in, the terminal driving it, a long call.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-002_record-call-in-progress.md` | — | — |
| 2 | `recording/TC-003_cancel-arm.md` | — | — |
| 3 | `recording/TC-004_arm-expires.md` | — | — |
| 4 | `recording/TC-005_stop-by-hand.md` | — | — |
| 5 | `recording/TC-006_drop-and-rejoin.md` | — | — |
| 6 | `recording/TC-007_switch-audio-device.md` | — | — |
| 7 | `recording/TC-008_speakers-echo.md` | — | — |
| 8 | `permissions/TC-006_accessibility.md` | — | — |
| 9 | `recording/TC-009_mute-in-teams.md` | 8 | — |
| 10 | `recording/TC-010_sleep-while-recording.md` | — | — |
| 11 | `recording/TC-011_crash-while-recording.md` | — | — |
| 12 | `recording/TC-012_ask-mode.md` | — | — |
| 13 | `privacy/TC-001_no-recording-without-opt-in.md` | — | — |
| 14 | `privacy/TC-002_only-teams-recorded.md` | — | — |
| 15 | `cli/TC-004_arm-stop-terminal.md` | — | — |
| 16 | `transcription/TC-006_long-call.md` | — | — |

## Notes
- Each stage is its own call; none depends on another except mute on Accessibility.

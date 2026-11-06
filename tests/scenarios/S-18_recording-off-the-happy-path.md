---
id: S-18
title: Recording off the happy path: mid-call starts, cancels, drops, devices, mute, sleep, crashes
run: person
data: small
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1) and BlackHole 2ch; the watcher cases start a second app instance on the run's own data folder and config (never `~/echo`); person-run stages use the tray, Teams and macOS dialogs
cases:
  - tests/cases/recording/TC-002_record-call-in-progress.md
  - tests/cases/recording/TC-003_cancel-arm.md
  - tests/cases/recording/TC-004_arm-expires.md
  - tests/cases/recording/TC-015_stop-by-hand-in-the-app.md
  - tests/cases/recording/TC-016_capture-stopped-by-hand.md
  - tests/cases/recording/TC-011_crash-while-recording.md
  - tests/cases/recording/TC-017_silence-in-a-recorded-call.md
  - tests/cases/recording/TC-018_two-remote-voices.md
  - tests/cases/privacy/TC-001_no-recording-without-opt-in.md
  - tests/cases/privacy/TC-002_only-teams-recorded.md
  - tests/cases/cli/TC-004_arm-stop-terminal.md
  - tests/cases/recording/TC-005_stop-by-hand.md
  - tests/cases/recording/TC-006_drop-and-rejoin.md
  - tests/cases/recording/TC-007_switch-audio-device.md
  - tests/cases/recording/TC-008_speakers-echo.md
  - tests/cases/permissions/TC-006_accessibility.md
  - tests/cases/recording/TC-009_mute-in-teams.md
  - tests/cases/recording/TC-010_sleep-while-recording.md
  - tests/cases/recording/TC-012_ask-mode.md
  - tests/cases/transcription/TC-006_long-call.md
---

# S-18: Recording off the happy path: mid-call starts, cancels, drops, devices, mute, sleep, crashes

**Environment:** this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1) and BlackHole 2ch; the watcher cases start a second app instance on the run's own data folder and config (never `~/echo`); person-run stages use the tray, Teams and macOS dialogs

**Story:** Everything that can happen to a recording: starting it mid-call, cancelling, an arm that expires, stopping by hand, a crash, silence, two voices, nothing recorded without opting in, only the call app recorded, the terminal driving it; then what only Teams or the machine can do: the stop dialog, a drop and rejoin, switching to headphones and back, speakers, mute, the lid closed, ask mode, a long call. Stages 1 to 11 run with the test call (agent); stages 12 to 20 need a person.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-002_record-call-in-progress.md` | — | — |
| 2 | `recording/TC-003_cancel-arm.md` | — | — |
| 3 | `recording/TC-004_arm-expires.md` | — | — |
| 4 | `recording/TC-015_stop-by-hand-in-the-app.md` | — | — |
| 5 | `recording/TC-016_capture-stopped-by-hand.md` | — | — |
| 6 | `recording/TC-011_crash-while-recording.md` | — | — |
| 7 | `recording/TC-017_silence-in-a-recorded-call.md` | — | — |
| 8 | `recording/TC-018_two-remote-voices.md` | — | — |
| 9 | `privacy/TC-001_no-recording-without-opt-in.md` | — | — |
| 10 | `privacy/TC-002_only-teams-recorded.md` | — | — |
| 11 | `cli/TC-004_arm-stop-terminal.md` | — | — |
| 12 | `recording/TC-005_stop-by-hand.md` | — | — |
| 13 | `recording/TC-006_drop-and-rejoin.md` | — | — |
| 14 | `recording/TC-007_switch-audio-device.md` | — | — |
| 15 | `recording/TC-008_speakers-echo.md` | — | — |
| 16 | `permissions/TC-006_accessibility.md` | — | — |
| 17 | `recording/TC-009_mute-in-teams.md` | 16 | — |
| 18 | `recording/TC-010_sleep-while-recording.md` | — | — |
| 19 | `recording/TC-012_ask-mode.md` | — | — |
| 20 | `transcription/TC-006_long-call.md` | — | — |

## Notes
- Each stage is its own call; none depends on another except mute on Accessibility.
- Stages that start the second app instance (1 to 4, 9, 11) quit it at their end, so they can run one after another.

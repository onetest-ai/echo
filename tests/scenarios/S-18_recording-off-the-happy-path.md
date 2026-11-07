---
id: S-18
title: Recording off the happy path: mid-call starts, cancels, expiry, stops, a crash, silence, two voices, opt-in, a drop, ask mode
run: agent
platform: macos
data: small
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens; person stages: Teams, devices, the room's speakers, the Mac's sleep, System Settings
cases:
  - tests/cases/recording/TC-002_record-call-in-progress.md
  - tests/cases/recording/TC-003_cancel-arm.md
  - tests/cases/recording/TC-004_arm-expires.md
  - tests/cases/recording/TC-015_stop-by-hand-in-the-app.md
  - tests/cases/recording/TC-005_stop-by-hand.md
  - tests/cases/recording/TC-016_capture-stopped-by-hand.md
  - tests/cases/recording/TC-011_crash-while-recording.md
  - tests/cases/recording/TC-017_silence-in-a-recorded-call.md
  - tests/cases/recording/TC-018_two-remote-voices.md
  - tests/cases/privacy/TC-001_no-recording-without-opt-in.md
  - tests/cases/privacy/TC-002_only-teams-recorded.md
  - tests/cases/cli/TC-004_arm-stop-terminal.md
  - tests/cases/recording/TC-006_drop-and-rejoin.md
  - tests/cases/recording/TC-012_ask-mode.md
  - tests/cases/recording/TC-007_switch-audio-device.md
  - tests/cases/recording/TC-008_speakers-echo.md
  - tests/cases/permissions/TC-006_accessibility.md
  - tests/cases/recording/TC-009_mute-in-teams.md
  - tests/cases/recording/TC-010_sleep-while-recording.md
  - tests/cases/transcription/TC-006_long-call.md
  - tests/cases/recording/TC-020_call-with-system-audio-denied.md
  - tests/cases/recording/TC-021_call-with-mic-denied.md
  - tests/cases/recording/TC-022_shared-screen-snapshots.md
---

# S-18: Recording off the happy path

**Environment:** this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens. How the run drives the test call: app_profile.md → "Recording with the test call".

**Data:** `small`, hermetic: `tests/fixtures/make_data.py <dir> --ai fake` (the app's own Claude Code jobs are refused, never sent to an account), the speech model linked into its config; for stage 9 the NeMo-Speech runtime fetched into its config (the case's Preconditions); for stage 5 the window served on it with clicks reaching the app.

**Story:** Everything that can happen to a recording: starting it mid-call, cancelling, an arm that expires, stopping from the terminal and from the window's pill, Ctrl-C, a crash, silence, two voices, nothing recorded without opting in, only the call app recorded, the terminal driving it, a drop and rejoin, Echo's own "Record?" dialog; then what only a person can do: switching devices, speakers, Accessibility and mute, the lid closed, an hour-long call, permissions denied, shared screens.

**Length:** about 20 minutes for stages 1–14 (one TestCall call each, 9–75 s, plus the waits for transcripts).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-002_record-call-in-progress.md` | — | — |
| 2 | `recording/TC-003_cancel-arm.md` | — | — |
| 3 | `recording/TC-004_arm-expires.md` | — | — |
| 4 | `recording/TC-015_stop-by-hand-in-the-app.md` | — | — |
| 5 | `recording/TC-005_stop-by-hand.md` | — | Serve the window on the run's data with `ECHOCTL_SERVE_IPC=1`; load `{{base_url}}/` |
| 6 | `recording/TC-016_capture-stopped-by-hand.md` | — | — |
| 7 | `recording/TC-011_crash-while-recording.md` | — | — |
| 8 | `recording/TC-017_silence-in-a-recorded-call.md` | — | — |
| 9 | `recording/TC-018_two-remote-voices.md` | — | Fetch the NeMo-Speech runtime into the run's config (its Preconditions) |
| 10 | `privacy/TC-001_no-recording-without-opt-in.md` | — | — |
| 11 | `privacy/TC-002_only-teams-recorded.md` | — | `afplay` plays out loud: say so to the user first |
| 12 | `cli/TC-004_arm-stop-terminal.md` | — | — |
| 13 | `recording/TC-006_drop-and-rejoin.md` | — | — |
| 14 | `recording/TC-012_ask-mode.md` | — | The run's terminal may use System Events |
| 15 | `recording/TC-007_switch-audio-device.md` | — | **person** |
| 16 | `recording/TC-008_speakers-echo.md` | — | **person** |
| 17 | `permissions/TC-006_accessibility.md` | — | **person** |
| 18 | `recording/TC-009_mute-in-teams.md` | 17 | **person** |
| 19 | `recording/TC-010_sleep-while-recording.md` | — | **person** |
| 20 | `transcription/TC-006_long-call.md` | — | **person** |
| 21 | `recording/TC-020_call-with-system-audio-denied.md` | — | **person** — System Audio Recording denied for Echo (permissions/TC-003, S-16) |
| 22 | `recording/TC-021_call-with-mic-denied.md` | — | **person** — Microphone denied for Echo (permissions/TC-004, S-16) |
| 23 | `recording/TC-022_shared-screen-snapshots.md` | — | **person** — "Save snapshots of shared screens" on (permissions/TC-007, S-16) |

## Notes
- Each stage is its own call. The watcher stages may keep one app running from stage to stage (each notes the events before it in `$D/ev.start` and counts only its own); stage 14 sets `--unscheduled ask` and back.
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why.
- Run 2026-11-07 on the dev Mac: stages 1–14 PASS (stage 9 after fetching the NeMo runtime; without it its steps 5–6 fail).

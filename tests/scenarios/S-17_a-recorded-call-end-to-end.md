---
id: S-17
title: A recorded call end to end: record, take notes, transcript, minutes, the files
run: agent
platform: macos
data: small
ai: local
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens; the minutes stages run Claude Code for real on the local model (`make_data.py <dir> --ai local`)
cases:
  - tests/cases/recording/TC-014_test-call-two-tracks.md
  - tests/cases/recording/TC-001_record-next-call.md
  - tests/cases/window/TC-014_notes-while-recording.md
  - tests/cases/window/TC-013_live-update-while-recording.md
  - tests/cases/privacy/TC-003_no-audio-kept.md
  - tests/cases/transcription/TC-004_pin-language.md
  - tests/cases/recording/TC-013_very-short-call.md
  - tests/cases/claude/TC-009_own-notes-in-minutes.md
  - tests/cases/claude/TC-003_auto-minutes.md
  - tests/cases/claude/TC-010_notes-move-minutes.md
  - tests/cases/window/TC-012_minutes-open-in-browser.md
  - tests/cases/recording/TC-019_tray-icon-follows-the-recording.md
---

# S-17: A recorded call end to end: record, take notes, transcript, minutes, the files

**Environment:** this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens. How the run drives the test call: app_profile.md → "Recording with the test call".

**Data:** `small` with the local model: `tests/fixtures/make_data.py <dir> --ai local` (hermetic: the app's own Claude Code jobs go to the local model, never an account), the speech model linked into its config, then the window served on it with `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` (after `. <dir>/env`). Needs Claude Code on `PATH` and the data's model server answering.

**Story:** Sam records a call, takes notes while it runs, watches it land in the window, and gets minutes that keep the notes, written by themselves 15 minutes after the call — later when he adds a note; a very short call gets a transcript only; the minutes open in the browser and in Finder.

**Length:** about 45 minutes, most of it the two automatic-minutes waits (15 minutes each, overlapping); TestCall calls are 9–213 s.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-014_test-call-two-tracks.md` | — | — |
| 2 | `recording/TC-001_record-next-call.md` | — | — |
| 3 | `window/TC-014_notes-while-recording.md` | — | Load `{{base_url}}/` (the cookie) |
| 4 | `window/TC-013_live-update-while-recording.md` | — | — |
| 5 | `privacy/TC-003_no-audio-kept.md` | — | — |
| 6 | `transcription/TC-004_pin-language.md` | — | — |
| 7 | `recording/TC-013_very-short-call.md` | — | — |
| 8 | `claude/TC-009_own-notes-in-minutes.md` | — | Its call ("MIN-A catch-up") is `claude/TC-003`'s: run stage 9's steps 4–5 right after its call, and its step 6 is stage 9's wait |
| 9 | `claude/TC-003_auto-minutes.md` | 8 | Same app and call as stage 8 (skip its steps 2–3) |
| 10 | `claude/TC-010_notes-move-minutes.md` | — | Record its call right after stage 8's call ends (same app); its wait overlaps stage 9's |
| 11 | `window/TC-012_minutes-open-in-browser.md` | — | — |
| 12 | `recording/TC-019_tray-icon-follows-the-recording.md` | — | **person** — skipped by an agent run (the tray can't be driven while the user's Echo runs) |

## Notes
- Stages 2–3 and 8–10 start the second app instance and quit it at their end; stage 4 records with `capture` (no app needed).
- Only one TestCall call at a time on a Mac (port 7792, BlackHole): when other runs share the Mac, their watchers see this run's calls (unarmed: nothing recorded) — never arm two runs at once.
- Run 2026-11-07 on the dev Mac (network model server): stages 1–11 run; PASS but stage 1 step 2 and stage 6 step 3 (`capture` exits 134 after saving, #141) and stage 7 step 4 (a line said in the room after the hang-up, the mic-fallback defect). Minutes runs: 116 s each.

---
id: S-19
title: Claude Code and the minutes: write now, your notes in them, one writer, folders, models, doctor, automatic minutes off, catch-up
run: agent
platform: macos
data: small
ai: local
clock: fast
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens; Claude Code runs for real on the local model (`make_data.py <dir> --ai local`); the calls are copies of the recordings prepared once for this build (`tests/fixtures/recordings.py`), so nothing is recorded or transcribed here and the app runs only where a stage starts it
cases:
  - tests/cases/claude/TC-004_write-minutes-now.md
  - tests/cases/claude/TC-009_own-notes-in-minutes.md
  - tests/cases/claude/TC-005_session-vs-echo.md
  - tests/cases/claude/TC-006_minutes-folder.md
  - tests/cases/claude/TC-012_minutes-model.md
  - tests/cases/claude/TC-015_auto-minutes-off.md
  - tests/cases/cli/TC-005_doctor.md
  - tests/cases/claude/TC-007_catch-up.md
  - tests/cases/claude/TC-002_doctor-session.md
  - tests/cases/claude/TC-008_live-pane.md
  - tests/cases/claude/TC-011_signed-out.md
  - tests/cases/claude/TC-014_record-from-chat.md
  - tests/cases/privacy/TC-007_no-send-without-yes.md
---

# S-19: Claude Code and the minutes

**Environment:** this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens.

**Data:** `small` with the local model and the test clock: `tests/fixtures/make_data.py <dir> --ai local --clock fast` (on small data only minutes runs reach the model: the app's preps, meeting types, task modes and thread syncs are refused at once and `project_sync` is off, so nothing queues in front of the minutes; automatic minutes 90 s after a call instead of 15 minutes), the window served on it with `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` (after `. <dir>/env`, so it has the same clock). Needs Claude Code on `PATH` and the data's model server answering.

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded calls"): every call here is a copy of its `minutes-call` (213 s) or `very-short`.

**Story:** Sam writes minutes now from the window — also for a call too short for automatic ones, and for one where his own notes must go in — while a session tries to take the same call; he points Echo's runs at another folder and back, gives a profile a model that doesn't exist and is told what to do; he checks `doctor` against Health; with automatic minutes off nothing is written until asked, and a call that ended while Echo was quit is caught up when it starts again. A signed-in session (doctor, the live pane, signing out, recording from a chat, follow-ups) is a person's part.

**Length:** about 15 minutes: six model runs (45–120 s each on the network server) and two waits of about 2 minutes on the test clock (stages 6 and 8). No TestCall call.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `claude/TC-004_write-minutes-now.md` | — | `config --notes-in "$D/folder-a"` (stage 4's step 1) before its step 4; `config --notes-in any` before its step 7 |
| 2 | `claude/TC-009_own-notes-in-minutes.md` | — | — |
| 3 | `claude/TC-005_session-vs-echo.md` | 1 | Its step 1 runs while stage 1's step 4 writes ("Writing minutes…") |
| 4 | `claude/TC-006_minutes-folder.md` | 1 | Its two runs are stage 1's (steps 4 and 7): read their logs |
| 5 | `claude/TC-012_minutes-model.md` | 1 | Its call is stage 1's short call |
| 6 | `claude/TC-015_auto-minutes-off.md` | — | — |
| 7 | `cli/TC-005_doctor.md` | 6 | While stage 6's app runs |
| 8 | `claude/TC-007_catch-up.md` | 6 | Stage 6's step 4 switches automatic minutes on: its next call is this one |
| 9 | `claude/TC-002_doctor-session.md` | — | **person** |
| 10 | `claude/TC-008_live-pane.md` | — | **person** |
| 11 | `claude/TC-011_signed-out.md` | — | **person** |
| 12 | `claude/TC-014_record-from-chat.md` | — | **person** |
| 13 | `privacy/TC-007_no-send-without-yes.md` | — | **person** |

## Notes
- Model runs: stage 1 (two), 2, 3, 6 and 8 — each one judges what a model writes or does; stage 5's wrong model fails at once. Stage 8 is the automatic run whose content and log `claude/TC-003` (S-17, on the fake) no longer checks.
- The minutes run's guard against the user's permissive settings (`claude/TC-013`) needs the hostile set: it is in S-31.
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why.

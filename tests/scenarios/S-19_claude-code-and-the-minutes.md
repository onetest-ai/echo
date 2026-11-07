---
id: S-19
title: Claude Code and the minutes: write now, one writer, folders, models, doctor, automatic minutes off, catch-up
run: agent
platform: macos
data: small
ai: local
env: this Mac: Echo's debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`, TestCall (both ends on this Mac: serve and dial on 127.0.0.1), BlackHole 2ch and ffmpeg; the watcher stages start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens; Claude Code runs for real on the local model (`make_data.py <dir> --ai local`); the calls are recorded with `capture`, so the app runs only where a stage starts it
cases:
  - tests/cases/claude/TC-004_write-minutes-now.md
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

**Data:** `small` with the local model: `tests/fixtures/make_data.py <dir> --ai local`, the speech model linked into its config, the window served on it with `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>`. Needs Claude Code on `PATH` and the data's model server answering.

**Story:** Sam writes minutes now from the window — also for a call too short for automatic ones — while a session tries to take the same call; he points Echo's runs at another folder and back, gives a profile a model that doesn't exist and is told what to do; he checks `doctor` against Health; with automatic minutes off nothing is written until asked, and a call that ended while Echo was quit is caught up when it starts again. A signed-in session (doctor, the live pane, signing out, recording from a chat, follow-ups) is a person's part.

**Length:** about 50 minutes, most of it the two 15–20-minute waits (stages 5 and 7), while other scenarios may use the Mac's TestCall in between (never armed at the same time). Model runs: 45–60 s each on the network server.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `claude/TC-004_write-minutes-now.md` | — | `config --notes-in "$D/folder-a"` (stage 3's step 1) before its step 4; `config --notes-in any` before its step 7 |
| 2 | `claude/TC-005_session-vs-echo.md` | 1 | Its step 1 runs while stage 1's step 4 writes ("Writing minutes…") |
| 3 | `claude/TC-006_minutes-folder.md` | 1 | Its two runs are stage 1's (steps 4 and 7): read their logs |
| 4 | `claude/TC-012_minutes-model.md` | 1 | Its call is stage 1's short call |
| 5 | `claude/TC-015_auto-minutes-off.md` | — | — |
| 6 | `cli/TC-005_doctor.md` | 5 | While stage 5's app runs |
| 7 | `claude/TC-007_catch-up.md` | 5 | Stage 5's step 3 switches automatic minutes on: its next call is this one |
| 8 | `claude/TC-002_doctor-session.md` | — | **person** |
| 9 | `claude/TC-008_live-pane.md` | — | **person** |
| 10 | `claude/TC-011_signed-out.md` | — | **person** |
| 11 | `claude/TC-014_record-from-chat.md` | — | **person** |
| 12 | `privacy/TC-007_no-send-without-yes.md` | — | **person** |

## Notes
- The minutes run's guard against the user's permissive settings (`claude/TC-013`) needs the hostile set: it is in S-31.
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why.

---
id: S-19
title: Claude Code and the minutes: write now, one writer, folders, catch-up, live pane, models, the guard
run: person
env: this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists and Claude Code signed in
cases:
  - tests/cases/claude/TC-002_doctor-session.md
  - tests/cases/cli/TC-005_doctor.md
  - tests/cases/claude/TC-004_write-minutes-now.md
  - tests/cases/claude/TC-005_session-vs-echo.md
  - tests/cases/claude/TC-006_minutes-folder.md
  - tests/cases/claude/TC-007_catch-up.md
  - tests/cases/claude/TC-008_live-pane.md
  - tests/cases/claude/TC-011_signed-out.md
  - tests/cases/claude/TC-012_minutes-model.md
  - tests/cases/claude/TC-013_run-only-touches-call.md
  - tests/cases/claude/TC-014_record-from-chat.md
  - tests/cases/claude/TC-015_auto-minutes-off.md
  - tests/cases/privacy/TC-007_no-send-without-yes.md
---

# S-19: Claude Code and the minutes: write now, one writer, folders, catch-up, live pane, models, the guard

**Environment:** this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists and Claude Code signed in

**Story:** Sam works with Claude Code around calls: doctor, writing minutes now, a session and Echo racing for the same call, the minutes folder, calls missed while quit, the live pane, signed out and back, models per profile, the guard against a hostile call, recording from a chat, automatic minutes off, and nothing sent without a yes.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `claude/TC-002_doctor-session.md` | — | — |
| 2 | `cli/TC-005_doctor.md` | — | — |
| 3 | `claude/TC-004_write-minutes-now.md` | — | — |
| 4 | `claude/TC-005_session-vs-echo.md` | — | — |
| 5 | `claude/TC-006_minutes-folder.md` | — | — |
| 6 | `claude/TC-007_catch-up.md` | — | — |
| 7 | `claude/TC-008_live-pane.md` | — | — |
| 8 | `claude/TC-011_signed-out.md` | — | — |
| 9 | `claude/TC-012_minutes-model.md` | — | — |
| 10 | `claude/TC-013_run-only-touches-call.md` | — | — |
| 11 | `claude/TC-014_record-from-chat.md` | — | — |
| 12 | `claude/TC-015_auto-minutes-off.md` | — | — |
| 13 | `privacy/TC-007_no-send-without-yes.md` | — | — |

## Notes
- Profiles under test may use Ollama qwen3.8 27B (mlx on the Mac) instead of a cloud model.

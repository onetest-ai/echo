---
id: S-22
title: Echo on Windows: build, install, record, the window, login, updates, mute, notifications, frames
run: person
env: a Windows 10 2004+/11 machine or VM with Teams and the MSVC toolchain
cases:
  - tests/cases/windows/TC-001_build.md
  - tests/cases/windows/TC-002_install-first-run.md
  - tests/cases/windows/TC-003_record-call.md
  - tests/cases/windows/TC-004_window.md
  - tests/cases/windows/TC-005_login-quit-path.md
  - tests/cases/windows/TC-006_update.md
  - tests/cases/windows/TC-007_mute-sync.md
  - tests/cases/windows/TC-008_notifications-tray.md
  - tests/cases/windows/TC-009_screen-frames.md
---

# S-22: Echo on Windows: build, install, record, the window, login, updates, mute, notifications, frames

**Environment:** a Windows 10 2004+/11 machine or VM with Teams and the MSVC toolchain

**Story:** The Windows build from source to a recorded call: package, install, first run, record with Teams, the window, sign-in start and quit, updates, mute, notifications and the tray, screen frames.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `windows/TC-001_build.md` | — | — |
| 2 | `windows/TC-002_install-first-run.md` | 1 | — |
| 3 | `windows/TC-003_record-call.md` | 2 | — |
| 4 | `windows/TC-004_window.md` | 2 | — |
| 5 | `windows/TC-005_login-quit-path.md` | 2 | — |
| 6 | `windows/TC-006_update.md` | 2 | — |
| 7 | `windows/TC-007_mute-sync.md` | 3 | — |
| 8 | `windows/TC-008_notifications-tray.md` | 2 | — |
| 9 | `windows/TC-009_screen-frames.md` | 3 | — |

## Notes
- AI profiles use Ollama qwen3.8:27b (no MLX on Windows).

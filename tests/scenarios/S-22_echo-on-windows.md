---
id: S-22
title: Echo on Windows: build, install, record, the window, login, updates, mute, notifications, frames
run: person
env: a Windows 10 2004+/11 machine with Teams and the MSVC toolchain
cases:
  - tests/cases/windows/TC-001_build.md
  - tests/cases/windows/TC-002_install-first-run.md
  - tests/cases/windows/TC-003_record-call.md
  - tests/cases/windows/TC-007_mute-sync.md
  - tests/cases/windows/TC-009_screen-frames.md
  - tests/cases/windows/TC-008_notifications-tray.md
  - tests/cases/windows/TC-004_window.md
  - tests/cases/windows/TC-005_login-quit-path.md
  - tests/cases/windows/TC-006_update.md
---

# S-22: Echo on Windows: build, install, record, the window, login, updates, mute, notifications, frames

**Environment:** a Windows 10 2004+/11 machine with Teams and the MSVC toolchain

**Story:** The Windows build from source to a recorded call: package, install, first run, record with Teams, the window, sign-in start and quit, updates, mute, notifications and the tray, screen frames.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The release build and its zip (one build) | stage 1 | 1, 2 |
| The install and first run (folder, model download) | stage 2 | 3–9 |
| **One recorded Teams call** with a second participant: 2 minutes both sides with a video playing, a ~20 s mute while talking, slides shared, snapshots on | stage 3 (steps 2–4) | 3, 4 (the mute), 5 (the frames), 6 (its toast), 7 (calls in the window) |
| One sign-out and sign-in | stage 8 | 8 |
| A newer release to update to | outside the run (published) | 9 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `windows/TC-001_build.md` | — | — |
| 2 | `windows/TC-002_install-first-run.md` | 1 | — |
| 3 | `windows/TC-003_record-call.md` | 2 | The scenario's one call. **Before its step 2:** Settings → "Save snapshots of shared screens" on (stage 5's step 2). **During its step 4:** `echoctl ax-mic --seconds 30` with a mute and an unmute (stage 4's step 1); stay muted ~20 s in the middle while talking (stage 4's step 2); the other side shares a few slides (stage 5's step 2). **After it:** click the toast of the finished recording (stage 6's step 1) |
| 4 | `windows/TC-007_mute-sync.md` | 3 | Its call is stage 3's: read step 1's output and step 2's events, transcript and files there |
| 5 | `windows/TC-009_screen-frames.md` | 3 | Step 1 as written; step 2's call is stage 3's: check `frames/` and `frames.jsonl` there |
| 6 | `windows/TC-008_notifications-tray.md` | 3 | Step 1's recording is stage 3's (its toast clicked then); `echoctl notify` as written |
| 7 | `windows/TC-004_window.md` | 3 | Its calls include stage 3's |
| 8 | `windows/TC-005_login-quit-path.md` | 2 | — |
| 9 | `windows/TC-006_update.md` | 2 | Last: it replaces the installed version |

## Notes
- **One recorded call** serves stages 3–7 (before 2026-11-08 stages 3, 7, 8 and 9 each recorded their own Teams call:
  four calls with a second participant). The mute, the slides and the toast don't change what stage 3 checks (both
  sides, the mic track Teams used, the video absent, no WAV left).
- AI profiles use Ollama qwen3.8:27b (no MLX on Windows).

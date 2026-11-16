---
id: S-21
title: Living with Echo: quit, one instance, another folder, appearance; login, updates, the native window, offline, the network
run: agent
platform: macos
data: small
env: this Mac: the dev build run as a second app beside the user's own Echo, on its own test data folder and config (never `~/echo`, never `app start`), `--scheduled manual`; the window served on the data
cases:
  - tests/cases/lifecycle/TC-004_single-instance.md
  - tests/cases/lifecycle/TC-003_quit.md
  - tests/cases/lifecycle/TC-005_change-data-folder.md
  - tests/cases/window/TC-015_appearance-follows-system.md
  - tests/cases/lifecycle/TC-002_start-at-login-off.md
  - tests/cases/cli/TC-006_app-start-after-quit.md
  - tests/cases/window/TC-011_native-window-dock.md
  - tests/cases/lifecycle/TC-007_update.md
  - tests/cases/lifecycle/TC-008_no-update-while-recording.md
  - tests/cases/lifecycle/TC-009_up-to-date.md
  - tests/cases/privacy/TC-004_offline-transcription.md
  - tests/cases/privacy/TC-005_network-destinations.md
auto: [tests/auto/specs/S-21.spec.mjs]
---

# S-21: Living with Echo

**Environment:** this Mac: the dev build run as a second app beside the user's own Echo, on its own test data and config (never `~/echo`); the window served on the data for stage 4.

**Data:** `small`, hermetic: `tests/fixtures/serve.sh up <dir> <port> --ai fake` (no thread sync of the app's own: `project_sync` off on small `--ai` data; its env feeds calls from files: `app_profile.md` → "Recording with file-fed calls", no TestCall or permission); the speech model linked into its config for stage 3's call (`recording/TC-001` step 1). The waits for the app are waits for its state (`tests/fixtures/wait.py`).

**Story:** Over days of use: a second start refused, quitting and coming back without first-run questions, moving the data folder, the window following the system's light and dark; then what needs an installed Echo, a login or the network: start at login, `app start`, the native window and its Dock icon, updates, offline, the network it talks to.

**Length:** about 2 minutes for stages 1–4 (CLI part measured 2026-11-08 evening: 47 s for stages 1–3).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small`, `--ai fake`) and the served window | the lead (`serve.sh up`) | all |
| The app on the run's data (`config --unscheduled manual --scheduled manual --auto-minutes false`, `ai live off`, `app run --no-tray --background`) | Prelude | 1, 2 (stage 2 quits it, starts it again and quits it: that is its subject) |
| Stage 3's app, started without `ECHOCTL_HOME` (the variable would pin the data folder), and its restart by hand after `home set` | stage 3 | 3 |
| **The one live call**: `very-short` (9 s) fed from files, armed in the new data folder, recorded and transcribed there by the app | stage 3 step 4 | 3 |

## Prelude

Done once, before stage 1, timed as the stage `prelude`. After `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"`:

| # | Action | Expected Result |
|---|---|---|
| P1 | `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `python3 tests/fixtures/wait.py 30 app` | "live notes during calls: off"; "waited … for app"; no "already running" in `$D/app.out` (else every stage that needs the app is BLOCKED) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `lifecycle/TC-004_single-instance.md` | prelude | The prelude's app. Skip step 3 (keep the app for stage 2: its step 1 quits it, with the same check) |
| 2 | `lifecycle/TC-003_quit.md` | prelude | The prelude's app, still running after stage 1 |
| 3 | `lifecycle/TC-005_change-data-folder.md` | — | No app running (stage 2 quit it); the speech model linked (`recording/TC-001` step 1) |
| 4 | `window/TC-015_appearance-follows-system.md` | — | Load `{{base_url}}/` (the lead's server) |
| 5 | `lifecycle/TC-002_start-at-login-off.md` | — | **person** |
| 6 | `cli/TC-006_app-start-after-quit.md` | — | **person** |
| 7 | `window/TC-011_native-window-dock.md` | — | **person** |
| 8 | `lifecycle/TC-007_update.md` | — | **person** — needs a newer release (the URL is fixed to GitHub) |
| 9 | `lifecycle/TC-008_no-update-while-recording.md` | — | **person** |
| 10 | `lifecycle/TC-009_up-to-date.md` | — | **person** |
| 11 | `privacy/TC-004_offline-transcription.md` | — | **person** |
| 12 | `privacy/TC-005_network-destinations.md` | — | **person** |

## Notes
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why — mostly that on a dev Mac the installed Echo, its LaunchAgent and its tray are the user's own.
- **One live call** (stage 3, the shortest conversation, 9 s) and the app's one transcription of it. Stage 3 can't share the app of stages 1–2: it must run without `ECHOCTL_HOME`, which an app started with the env's variable keeps for its life. Stages 1–2 share one app (stage 1 no longer quits it for stage 2 to start it again).
- Before 2026-11-08 evening: the same one call, through TestCall and BlackHole; five app starts (now four: the prelude, stage 2's restart and stage 3's two — those restarts are what the stages check).
- Run 2026-11-08 evening, CLI part (no browser: stage 4 not run): prelude 1.1 s, stage 1 0.06 s, stage 2 3.2 s, stage 3 42 s (the call's folder and transcript in `home2` 24 s after it started). Stage 2 step 3 FAILED on `up`'s `ready` (`failing` `["system-audio"]`: the agent's shell has no System Audio Recording, not needed by file-fed calls): the case's expected result now allows it. Stage 3 again found the restart going to the installed Echo.app (#142; `open.log` only, `ECHOCTL_TEST_OPEN_LOG_ONLY=1`).
- Run 2026-11-07 on the dev Mac: stages 1–4 PASS; stage 3 found the dev build's restart going to the installed Echo.app (reported).

---
id: S-18
title: Recording off the happy path: cancels, expiry, opt-in, Ctrl-C, a crash, mid-call starts, stops, ask mode, a drop, silence, two voices
run: agent
platform: macos
data: small
ai: fake
env: this Mac: Echo's debug build on test data, calls fed from files (no device opened, no sound played, no TestCall); one second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--unscheduled manual --scheduled manual` (no "Record?" dialog but the ask stage's own); the live-check stages (person): TestCall, BlackHole 2ch and BlackHole 16ch; the other person stages: Teams, devices, the room's speakers, the Mac's sleep, System Settings
cases:
  - tests/cases/recording/TC-003_cancel-arm.md
  - tests/cases/recording/TC-004_arm-expires.md
  - tests/cases/privacy/TC-001_no-recording-without-opt-in.md
  - tests/cases/recording/TC-016_capture-stopped-by-hand.md
  - tests/cases/recording/TC-011_crash-while-recording.md
  - tests/cases/recording/TC-002_record-call-in-progress.md
  - tests/cases/cli/TC-004_arm-stop-terminal.md
  - tests/cases/recording/TC-015_stop-by-hand-in-the-app.md
  - tests/cases/recording/TC-005_stop-by-hand.md
  - tests/cases/recording/TC-012_ask-mode.md
  - tests/cases/recording/TC-006_drop-and-rejoin.md
  - tests/cases/recording/TC-017_silence-in-a-recorded-call.md
  - tests/cases/recording/TC-018_two-remote-voices.md
  - tests/cases/privacy/TC-002_only-teams-recorded.md
  - tests/cases/recording/TC-023_mic-stays-closed-after-the-hang-up.md
  - tests/cases/recording/TC-007_switch-audio-device.md
  - tests/cases/recording/TC-008_speakers-echo.md
  - tests/cases/permissions/TC-006_accessibility.md
  - tests/cases/recording/TC-009_mute-in-teams.md
  - tests/cases/recording/TC-010_sleep-while-recording.md
  - tests/cases/transcription/TC-006_long-call.md
  - tests/cases/recording/TC-020_call-with-system-audio-denied.md
  - tests/cases/recording/TC-021_call-with-mic-denied.md
  - tests/cases/recording/TC-022_shared-screen-snapshots.md
  - tests/cases/recording/TC-025_stop-while-echo-is-busy.md
  - tests/cases/transcription/TC-010_on-speakers-the-echo-is-skipped-and-the-users-words-kept.md
  - tests/cases/transcription/TC-012_a-calls-language-changed-during-it.md
auto: [tests/auto/specs/S-18.spec.mjs]
---

# S-18: Recording off the happy path

**Environment:** this Mac: Echo's debug build on test data; every call is fed from files (app_profile.md → "Recording
with file-fed calls": `tests/fixtures/call.py`), so the run opens no device, plays no sound and needs no TestCall,
BlackHole or permission. One second app instance on the run's own data folder and config (never `~/echo`, never
`app start`). Stages 14–15 are the live check (app_profile.md → "The live check"), run by a person.

**Data:** `small`, hermetic: `tests/fixtures/serve.sh up <dir> <port> --ai fake` (the app's own Claude Code jobs are
refused, never sent to an account), the window served on it with clicks reaching the app (`ECHOCTL_SERVE_IPC=1`).

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded
calls"): stages 12 and 13 read its `long-silence` and `two-remotes` instead of capturing and transcribing their own.

**Story:** Everything that can happen to a recording: an arm cancelled and an arm that expires, then a call that
nobody opted in to; a capture stopped with Ctrl-C and one that crashes, beside the app; a recording started in the
middle of the call; the terminal arming and stopping, and the call going on unrecorded after the stop; the window's
pill asking before it stops; Echo's own "Record?" dialog, and a drop and rejoin of the call it recorded; silence and
two voices; then what only a person can do: the real tap and microphone, devices, speakers, Accessibility and mute,
the lid closed, an hour-long call, permissions denied, shared screens.

**Length:** about 6.5 minutes for the prelude and stages 1–13 (388 s on 2026-11-08), plus the data build (14 s): one app
start, five calls the watcher sees (nine conversations of 9–36 s played into them), five transcriptions; every wait
for the app is a wait for its state (`tests/fixtures/wait.py`).

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The data (`small`, `--ai fake`) and the served window | the lead: `serve.sh up <dir> <port> --ai fake` | all; the window: 9 |
| The prepared recordings `long-silence`, `two-remotes` | `recordings.py prepare`, once per build | 12, 13 |
| The speech model linked, `config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `ai live off` | the prelude | 1–11 |
| **The app** (one instance: the watcher, its transcriptions) | the prelude; stage 11's teardown quits it | 1–11 |
| **The main call** (one call for the watcher, `--hold`: it begins unarmed, four conversations are played into it, each one a stage's timeline) | stage 3 starts it; stages 4, 5 and 6 start a new conversation in it; stage 6 hangs up | 1–2 (the call that follows their arms), 3–6 |
| The capture stopped with Ctrl-C (its tracks; no transcription) | stage 4, beside the app, in the main call | 4 |
| The crashed capture, repaired and transcribed by `echoctl transcribe` (transcription 1) | stage 5, in the main call | 5 |
| The recording armed mid-call, transcribed by the app (transcription 2) | stage 6, in the main call | 6 |
| **The terminal's call** (armed before it, stopped with `echoctl stop` 20 s in; it goes on unrecorded), transcribed by the app (transcription 3) | stage 7; stage 8 hangs up | 7, 8 |
| The pill's call (armed before it, stopped from the window), transcribed by the app (transcription 4) | stage 9 | 9 |
| **The ask calls** (`--unscheduled ask`): one answered "Not now", one "Record" — that recording dropped and rejoined, transcribed by the app (transcription 5) | stage 10 (both calls); stage 11 (the drop, the rejoin, the hang-up) | 10, 11 |
| The live check's TestCall calls (real devices) | a person | 14, 15 |

## Prelude

Timed as the stage `prelude`. Each Bash call is a fresh shell: start it with
`. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D` ("the
prelude" in every case below).

1. `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"; target/debug/echoctl model use large-v3-turbo-q5_0; target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false; target/debug/echoctl ai live off` — each exits 0.
2. `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app; target/debug/echoctl status` — "waited … for app"; `watcher` has a `pid`, `recording` null, `armed` false; no "already running" in `$D/app.out` (else the scenario is BLOCKED).
3. Load `{{base_url}}/` (the cookie for stage 9's clicks).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `recording/TC-003_cancel-arm.md` | prelude | The prelude's app: skip step 1 |
| 2 | `recording/TC-004_arm-expires.md` | prelude | The prelude's app: skip step 1 |
| 3 | `privacy/TC-001_no-recording-without-opt-in.md` | prelude | The prelude's app: skip step 1. Stages 1–2 left a cancelled and an expired arm: skip step 2. **Skip its teardown**: the main call goes on |
| 4 | `recording/TC-016_capture-stopped-by-hand.md` | 3 | In the main call (step 1's `call.py start` plays a new conversation into it); the app runs beside the capture. **Skip its teardown** |
| 5 | `recording/TC-011_crash-while-recording.md` | 3 | In the main call, as stage 4. **Skip its teardown** |
| 6 | `recording/TC-002_record-call-in-progress.md` | 3 | The prelude's app: skip step 1. In the main call, as stage 4; its step 4 hangs up the main call |
| 7 | `cli/TC-004_arm-stop-terminal.md` | prelude | The prelude's app: skip step 1. **Skip its teardown**: the call goes on for stage 8, and `$D/ev.start` stays as step 2 wrote it |
| 8 | `recording/TC-015_stop-by-hand-in-the-app.md` | 7 | On stage 7's call: `T=cli-004-terminal`; skip steps 1–3 (the app, the arm, the call and the stop at 20 s: stage 7 did them and checked their output) |
| 9 | `recording/TC-005_stop-by-hand.md` | prelude | The prelude's app and page: skip step 1 |
| 10 | `recording/TC-012_ask-mode.md` | prelude | The prelude's app: skip step 1. The run's terminal may click Echo's own dialog through System Events (by its pid only). **Skip its teardown**: the second call and its recording go on for stage 11 |
| 11 | `recording/TC-006_drop-and-rejoin.md` | 10 | On stage 10's recording ("Record" in the dialog): skip steps 1–2 (the app; the arm and the call); write its folder as step 2 does: `target/debug/echoctl status | python3 -c 'import json,sys;print(json.load(sys.stdin)["watcher"]["recording"])' > "$D/rec.call"`. Its teardown quits the app |
| 12 | `recording/TC-017_silence-in-a-recorded-call.md` | — | — |
| 13 | `recording/TC-018_two-remote-voices.md` | — | — |
| 14 | `privacy/TC-002_only-teams-recorded.md` | — | **person** — the live check (real tap, real devices) |
| 15 | `recording/TC-023_mic-stays-closed-after-the-hang-up.md` | — | **person** — the live check (#140) |
| 16 | `recording/TC-007_switch-audio-device.md` | — | **person** |
| 17 | `recording/TC-008_speakers-echo.md` | — | **person** |
| 18 | `permissions/TC-006_accessibility.md` | — | **person** |
| 19 | `recording/TC-009_mute-in-teams.md` | 18 | **person** |
| 20 | `recording/TC-010_sleep-while-recording.md` | — | **person** |
| 21 | `transcription/TC-006_long-call.md` | — | **person** |
| 22 | `recording/TC-020_call-with-system-audio-denied.md` | — | **person** — System Audio Recording denied for Echo (permissions/TC-003, S-16) |
| 23 | `recording/TC-021_call-with-mic-denied.md` | — | **person** — Microphone denied for Echo (permissions/TC-004, S-16) |
| 24 | `recording/TC-022_shared-screen-snapshots.md` | — | **person** — "Save snapshots of shared screens" on (permissions/TC-007, S-16) |
| 25 | `recording/TC-025_stop-while-echo-is-busy.md` | — | After stage 11's teardown quit the prelude's app: step 1 starts its own (it freezes it with `kill -STOP`) |
| 26 | `transcription/TC-010_on-speakers-the-echo-is-skipped-and-the-users-words-kept.md` | — | After stage 25's teardown quit its app: step 1 starts its own. A 126 s call on laptop speakers (a made-up echo), about 2.5 minutes |
| 27 | `transcription/TC-012_a-calls-language-changed-during-it.md` | — | After stage 26's teardown quit its app: step 1 starts its own. A 15 s Ukrainian call recorded as English, its language changed once both sides were transcribed |

## Notes
- **Why five calls, not one.** A stop by hand ends the recording and the rest of that call is never recorded again
  (that is what stages 8 and 9 check), so each stop by hand needs a call of its own, and the ask mode is another
  setting; the rest hangs off the main call. Each stage's timeline (arm 13 s in, a stop 20 s in, Ctrl-C after 20 s, a
  kill after 15 s) is one Bash command: a runner takes seconds between steps, so no timeline spans two stages. A
  conversation played into a call that is on already (`call.py end` and `call.py start` back to back, well within the
  watcher's 2 s debounce, `MIC_RELEASE_DEBOUNCE`) is the same call for the watcher, with the conversation's clock
  starting again: a recording or capture, open already or opened after it, hears it from the start (a source reopens
  on the new files when the call file is made anew, `audio/testfeed.rs`).
- Moved on 2026-11-08: "the call after the arm isn't recorded" from `recording/TC-003` and `TC-004` to
  `privacy/TC-001` (stage 3: one call after both arms); the arm's message and the stop's output from
  `recording/TC-015` to `cli/TC-004` (stage 7, the same recording); `privacy/TC-002` became the live check (`run:
  person`: file sources replace the tap it is about); `recording/TC-023` (#140) is new, person.
- No stage scores words but 26: transcription quality is S-14's. The app transcribes each call its watcher records (that
  is the app); the stages look only at what the recording covers. Stage 26 scores the call on speakers because what it
  checks happens only in the app's live transcription (the echo skipped during the call, #238).
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why.
- Run 2026-11-08 on the dev Mac (this plan, CLI parts, fresh `small --ai fake` data, one app): prelude 1.1 s, stages
  1–13: 6.0, 5.0, 7.4, 27.5, 24.7, 53.3, 25.9, 20.2, 42.3, 17.8, 97.8, 0.2, 0.1 s (388 s from the prelude's start to
  stage 13's end). All PASS but stage 11 step 5 (the words after the rejoin: 0 — the sources didn't follow a new call file then;
  fixed the same day, and a rejoin checked by hand carried both conversations into one folder) and stage 9
  steps 3–4 (the pill's "Stop recording?" and "Keep recording", page-only: not run without a browser; its "Stop" was
  sent as the page sends it, `POST /ipc {"cmd":"stop"}`, and passed). The back-to-back conversations in the main call
  gave no `call_ended`; the captures beside the app left the watcher idle; Echo's dialog was clicked by its pid.
- Run 2026-11-07 on the dev Mac (the old plan: every stage its own TestCall call and its own app start): stages 1–14
  PASS (stage 9 after fetching the NeMo runtime). Run 003's stages 1–11 took 55.9, 16.5, 17.4, 44.7, 44.4, 39.7, 42.4,
  0.2, 0.1, 28.5, 49.9 s.

---
id: S-19
title: Claude Code and the minutes: write now, your notes in them, one writer, folders, models, doctor, automatic minutes off, catch-up
run: agent
platform: macos
data: small
ai: local
clock: fast
env: this Mac: Echo's debug build; Claude Code runs for real on the local model (`make_data.py <dir> --ai local --clock fast`); the calls are copies of the recordings prepared once for this build (`tests/fixtures/recordings.py`), so nothing is recorded or transcribed here (no TestCall, BlackHole or ffmpeg); stages 6–8 start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens
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

**Environment:** this Mac: Echo's debug build; nothing is recorded or transcribed (no TestCall, BlackHole or ffmpeg): every call is a copy of a recording prepared once for this build. Stages 6–8 start a second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens.

**Data:** `small` with the local model and the test clock: `tests/fixtures/make_data.py <dir> --ai local --clock fast` (on small data only minutes runs reach the model: the app's preps, meeting types, task modes and thread syncs are refused at once and `project_sync` is off, so nothing queues in front of the minutes; automatic minutes 90 s after a call or the last note instead of 15 minutes), the window served on it with `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` (after `. <dir>/env`, so it has the same clock; its output is "the server's output" stage 3 reads). Needs Claude Code on `PATH` and the data's model server answering.

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded calls"): every call here is a copy of its `minutes-call` (213 s) or `very-short`.

**Story:** Sam writes minutes now from the window — with his own notes in them, and also for a call too short for automatic ones — while a session tries to take the same call, and takes a call back from a session; he points Echo's runs at another folder and back, gives a profile a model that doesn't exist and is told what to do; with automatic minutes off nothing is written until asked; he checks `doctor` against Health; and a call that ended while Echo was quit is caught up when it starts again. A signed-in session (doctor, the live pane, signing out, recording from a chat, follow-ups) is a person's part.

**Length:** about 30 minutes: four model runs (5–7 minutes each on the local model, 1–2 on the network server), one that fails at once (stage 5), and two checks that nothing happens (120 s in stage 6, 105 s in stage 8) on the test clock.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build (`--ai local --clock fast`) and the served window | the lead (`serve.sh up`) | 1–8 |
| The run's settings: automatic minutes on, both record modes manual, live notes off, folder A, the speech model linked | Prelude | 1–8 (folder A: 1, 4; the speech model: 7) |
| Long call "CLA-004 write now" (a copy of `minutes-call`) with the user's three notes in `my-notes.md`, its stamp in `$D/min-a.stamp` | stage 1 step 1, the notes right after it | 1, 2, 3, 4 |
| Short call "CLA-004 short" (a copy of `very-short`) | stage 1 step 2 | 1, 3, 4, 5 |
| **Model run A**: Write now on the long call, in folder A | stage 1 step 4 | 1 (writing, log, minutes), 2 (the notes in the minutes), 3 (a session refused while it writes), 4 (folder A in its log) |
| A session's claim on the short call | stage 3 step 2 (after stage 1's step 6) | 1 (step 7 takes it over), 3 |
| **Model run B**: Write minutes on the short call, in the data folder, taking over the session's claim | stage 1 step 7 | 1 (written anyway), 3 (the claim taken over), 4 (the data folder in its log), 5 (the model in its log and note) |
| The wrong-model run (fails at once, no model time) | stage 5 step 2 | 5 |
| Call "CLA-015 auto off" (a copy of `minutes-call`) | stage 6 step 2 | 6 |
| The dev app (one start) | stage 6 step 2 | 6, 7; quit by stage 8's step 1 |
| **Model run C**: Write minutes on the call that ended with automatic minutes off | stage 6 step 5 | 6 |
| Call "CLA-007 missed" (a copy of `minutes-call`, copied with Echo quit) | stage 8 step 2 | 8 |
| **Model run D**: the automatic catch-up run at the app's next start | stage 8 step 4 | 8 (the start, `pending`, the minutes, the log) |

## Prelude

Done once, before stage 1, timed as the stage `prelude`. Every Bash call starts with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D`.

| # | Action | Expected Result |
|---|---|---|
| P1 | Load `{{base_url}}/` (the cookie) in the open browser | The window loads |
| P2 | `target/debug/echoctl config --auto-minutes true --unscheduled manual --scheduled manual; target/debug/echoctl ai live off` | `auto_minutes` true; both record modes manual; live notes off (stage 6's app starts no live notes) |
| P3 | `mkdir -p "$D/folder-a/.claude"; echo '{"env": {"ECHO_TEST_FOLDER": "A"}}' > "$D/folder-a/.claude/settings.json"` | Folder A (claude/TC-006's precondition) |
| P4 | `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"; target/debug/echoctl model use large-v3-turbo-q5_0` | The speech model in use (cli/TC-005's step 3 names it; nothing is transcribed) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `claude/TC-004_write-minutes-now.md` | — | Stages 2–4 hang off its two runs. **Right after its step 1:** put stage 2's notes into that call and save its stamp: `CALL=<step 1's folder>; printf -- '- [00:09] ask legal about the contract\n- [00:14] Berlin needs ten seats in the training room\n- Ask Kim for the pilot budget\n' > "$CALL/my-notes.md"; basename "$CALL" \| cut -c1-17 > "$D/min-a.stamp"` (claude/TC-009's step 1 on this call; it also starts the 90 s again: click step 4's Write now within them). **Before its step 4:** `config --notes-in "$D/folder-a"` (stage 4's step 1). **While its step 4 writes** ("Writing minutes…"): stage 3's step 1. **After its step 6:** `config --notes-in any` (stage 4's step 3), then stage 3's step 2 on the short call (in place of Initech planning). Its step 7 is stage 3's step 3: the Minutes tab then says "A Claude Code session is writing the minutes", with the same "Write minutes" |
| 2 | `claude/TC-009_own-notes-in-minutes.md` | 1 | On stage 1's long call ("CLA-004 write now", stamp in `$D/min-a.stamp`): skip step 1 (the copy and the notes: stage 1's glue) and step 3 (the Write now: run A, stage 1's step 4). Steps 2, 4 and 5 check: My notes, the minutes written, the notes in them |
| 3 | `claude/TC-005_session-vs-echo.md` | 1 | Its steps ran inside stage 1: step 1 during run A, with `--for "$D/folder-a"` (Echo's runs are in folder A then: a session elsewhere is refused for that first); step 2 on the short call; step 3's click is stage 1's step 7 (run B). Here: `grep "claim taken over by Write minutes" "$D/serve.log"` (the server's output, `serve.sh`) and record the three steps' results |
| 4 | `claude/TC-006_minutes-folder.md` | 1 | Its steps 1 and 3's `config` ran in stage 1's glue, its two runs are stage 1's (run A in folder A, run B in the data folder): skip the Write minutes of steps 2–3, read both `claude.log` headers |
| 5 | `claude/TC-012_minutes-model.md` | 1 | Its call is stage 1's short call (run B) |
| 6 | `claude/TC-015_auto-minutes-off.md` | — | Skip its teardown: the app stays for stage 7, and stage 8's step 1 quits it |
| 7 | `cli/TC-005_doctor.md` | 6 | On stage 6's app (still running) |
| 8 | `claude/TC-007_catch-up.md` | 6 | Stage 6's step 4 switched automatic minutes on: its call is the first after that. Its step 1 quits stage 6's app |
| 9 | `claude/TC-002_doctor-session.md` | — | **person** |
| 10 | `claude/TC-008_live-pane.md` | — | **person** |
| 11 | `claude/TC-011_signed-out.md` | — | **person** |
| 12 | `claude/TC-014_record-from-chat.md` | — | **person** |
| 13 | `privacy/TC-007_no-send-without-yes.md` | — | **person** |

## Notes
- **Four model runs, one per condition that truly differs:** Write now on a call due for automatic minutes (A), Write minutes on a call the gate skipped and a session held (B), Write minutes on a call that ended while automatic minutes were off (C), and the automatic catch-up after a quit (D). Every check that can read a run reads one of those: the user's notes (stage 2) are in run A's call, the session's claim (stage 3) is refused during run A and taken over by run B, the two folders (stage 4) are runs A and B, the model (stage 5) is run B's. Stage 5's wrong model fails before any model time. Before 2026-11-08 the scenario started six runs: stage 2 wrote its own call's minutes and stage 3 its own (Initech planning).
- **Order inside stage 1:** stage 1 runs its steps in order and does stages 3–4's steps where its "Before it" says; stages 2–5 then do only what is left (their checks on runs A and B). The countdown: the long call is due 90 s after its notes (the test clock), and with no app running nothing writes it by itself, but the tab shows "Starting the minutes…" instead of Write now once that time has passed — keep steps 1–4 within the 90 s.
- **One app** (app_profile.md → "Recording with file-fed calls"): stage 6 starts it, stage 7 uses it, stage 8 quits it (its step 1) and starts it again (its step 4: the start is its subject), and its teardown quits it.
- Stage 8 is the automatic run whose content and log `claude/TC-003` (S-17, on the fake) no longer checks.
- If a stage 1 run fails, stages 2–5 that read it are BLOCKED (Depends on 1); stages 6–8 don't need it.
- The minutes run's guard against the user's permissive settings (`claude/TC-013`) needs the hostile set: it is in S-31.
- **person** stages are skipped by an agent run (the lead lists them as skipped (person)): each case's Notes say why.

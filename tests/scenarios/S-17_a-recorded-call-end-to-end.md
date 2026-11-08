---
id: S-17
title: A recorded call end to end: record, take notes, transcript, automatic minutes, the files
run: agent
platform: macos
data: small
ai: fake
clock: fast
env: this Mac: Echo's debug build on file-fed test data (no device, no sound, no permission); the recordings of this build prepared (`tests/fixtures/recordings.py prepare`, once per run); one second app instance on the run's own data folder and config (never `~/echo`, never `app start`), started in the prelude with `--scheduled manual` so no "Record?" dialog opens
cases:
  - tests/cases/window/TC-014_notes-while-recording.md
  - tests/cases/window/TC-013_live-update-while-recording.md
  - tests/cases/recording/TC-001_record-next-call.md
  - tests/cases/recording/TC-014_test-call-two-tracks.md
  - tests/cases/privacy/TC-003_no-audio-kept.md
  - tests/cases/transcription/TC-004_pin-language.md
  - tests/cases/recording/TC-013_very-short-call.md
  - tests/cases/claude/TC-003_auto-minutes.md
  - tests/cases/claude/TC-010_notes-move-minutes.md
  - tests/cases/window/TC-012_minutes-open-in-browser.md
  - tests/cases/recording/TC-019_tray-icon-follows-the-recording.md
---

# S-17: A recorded call end to end: record, take notes, transcript, automatic minutes, the files

**Environment:** this Mac: Echo's debug build; calls are fed from files (`app_profile.md` → "Recording with file-fed calls": no TestCall, BlackHole, ffmpeg or permission); one second app instance on the run's own data folder and config (never `~/echo`, never `app start`), with `--scheduled manual` so no "Record?" dialog opens.

**Before the run (once, not per scenario):** `tests/fixtures/recordings.py prepare` (app_profile.md → "Recorded calls"). Stages 6–9 copy its calls and transcribe nothing; stages 4–5 also read what its `acme-sync` capture printed.

**Data:** `small` on the fake and the test clock: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 tests/fixtures/serve.sh up <dir> <port> --ai fake --clock fast` (hermetic: every Claude Code run is refused at once; automatic minutes 90 s after a call or the last note instead of 15 minutes; the server inherits `ECHOCTL_TEST_OPEN_LOG_ONLY`, so `window/TC-012`'s `open` only writes `<dir>/open.log`).

**Story:** Sam arms Echo and the next call records itself while he takes notes in the window; he hangs up and watches the call fill in by itself — a transcript of both tracks, no audio left. Then, from calls recorded before: a language pinned for one call, a call too short for automatic minutes, the minutes starting by themselves at the time Echo named — later when he adds a note — and the minutes opening in the browser and in Finder. What a model writes in them is S-19's.

**Length:** about 8 minutes of CLI and waits (one 37–60 s call, two automatic-minutes waits of 90 s, a note's 90 s more), plus the window steps.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`small`, `--ai fake --clock fast`) and the served window | the lead (`serve.sh up`) | all |
| Prepared recordings of this build (`acme-sync`'s capture evidence; copies of `mixed-languages-pinned`, `mixed-languages`, `very-short`, `minutes-call` ×2) | the lead (`recordings.py prepare`, once per build) | 4, 5, 6, 7, 8, 9 |
| The app (one second instance, the speech model linked, `--unscheduled manual --scheduled manual --auto-minutes false`, `ai live off`) | Prelude P2–P3 (recording/TC-001 steps 1–2) | 1–9; quit by stage 9's teardown |
| The window's view: the page loaded once, Initech planning open on its Transcript tab before the call starts | Prelude P1 (window/TC-013 step 1) | 1, 2 (never reloaded until stage 2 ends) |
| **The main call**: `acme-sync`, armed as "REC-001 next call", `call.py start acme-sync --hold`, listed while recording (`$D/rec-001.during`), hung up in stage 1, recorded by the watcher and transcribed by the app — the one live call and the one transcription | Prelude P4–P5 (recording/TC-001 steps 3–4); hang-up: stage 1 step 5 | 1 (during, then the hang-up), 2, 3, 4, 5 (after) |
| Automatic-minutes waits on the test clock (90 s, then 90 s from a note) | stages 8 and 9 | 8, 9 |

## Prelude

Done once, before stage 1, timed as the stage `prelude`. Every Bash call starts with the prelude of `recording/TC-001` (`. <dir>/env; D=…; C=tests/fixtures/call.py; W=tests/fixtures/wait.py`).

| # | Action | Expected Result |
|---|---|---|
| P1 | Load `{{base_url}}/` (the cookie); then window/TC-013 step 1: `#calls`, Initech planning, its Transcript tab. Don't reload the page until stage 2 ends | window/TC-013 step 1's result |
| P2 | recording/TC-001 step 1 (the speech model linked, `model use`, `config --languages en`) | Its result |
| P3 | recording/TC-001 step 2 (`ev.start`, the app run with `--no-tray --background`, `$W 30 app`, `status`) | Its result; the app keeps running until stage 9's teardown |
| P4 | recording/TC-001 step 3 (arm "REC-001 next call") | Its result |
| P5 | recording/TC-001 step 4 **with `--hold`**: `$C start acme-sync --hold`, `$W 20 recording rec-001-next-call`, `status`, the two listings into `$D/rec-001.during` | Its result: the main call is recording; it stays on until stage 1 step 5 hangs it up |

Record P1–P5's results under the stages whose case steps they are (window/TC-013 step 1; recording/TC-001 steps 1–4): they are those cases' checks, made once.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-014_notes-while-recording.md` | prelude | `<slug>` = `rec-001-next-call`. Step 1: only `$W 5 recording rec-001-next-call` (the main call). Step 5 is the main call's hang-up. Skip its teardown (the app stays; leave the window on this call) |
| 2 | `window/TC-013_live-update-while-recording.md` | prelude, 1 | `<slug>` = `rec-001-next-call`. Step 1 was P1 and step 2 was P3–P5: check its results there. Step 3: on the page not reloaded since P1, the row is in the list (stage 1 step 2 opened it from the list while it said Recording: that is this check, made there). Step 4: on stage 1's view, skip the click. Skip its teardown |
| 3 | `recording/TC-001_record-next-call.md` | prelude, 1 | Steps 1–4 were P2–P5: take their results from there; go on from step 5. Skip its teardown (the app stays) |
| 4 | `recording/TC-014_test-call-two-tracks.md` | prelude, 1 | — (it reads the main call and `$D/rec-001.during`) |
| 5 | `privacy/TC-003_no-audio-kept.md` | prelude, 1 | — (it reads the main call and `$D/rec-001.during`) |
| 6 | `transcription/TC-004_pin-language.md` | — | — |
| 7 | `recording/TC-013_very-short-call.md` | — | — (the app keeps running; nothing here needs it) |
| 8 | `claude/TC-003_auto-minutes.md` | prelude | The scenario's app: step 1 without `nohup … app run …` / `$W 30 app` (`$W 5 app` instead). Skip its teardown (stage 9 goes on with the same app) |
| 9 | `claude/TC-010_notes-move-minutes.md` | 8 | The same app; its teardown quits it (the last stage that needs it) |
| 10 | `window/TC-012_minutes-open-in-browser.md` | — | — (the server was started with `ECHOCTL_TEST_OPEN_LOG_ONLY=1`) |
| 11 | `recording/TC-019_tray-icon-follows-the-recording.md` | — | **person** — skipped by an agent run (the tray can't be driven while the user's Echo runs) |

## Notes
- **One live call, one transcription, one app.** The main call is armed and started in the prelude and held (`--hold`) while the window takes notes on it; stage 1 hangs it up no earlier than 37 s in (the whole conversation recorded, however long the window steps took). Everything before (armed), during (the notes' time, both tracks written, the row Recording) and after it (the stop at the hang-up, the events, `stop_reason`, the transcript, two aligned closed tracks, no audio left, the window filling in) hangs off that call. Stages 6–9 copy prepared calls on the same app; nothing else records or transcribes. Transcription quality is scored once, in S-14.
- Before 2026-11-08 evening: three live TestCall calls (stages 2, 3, 5: 36 + 36 + 9 s; the last one a `capture` given a prepared transcript), the capture of stage 1 and the no-audio check read from `prepare`, three app starts (stages 2, 3 and 8: each watcher stage its own), two transcriptions by the app (stages 2 and 3). The checks are the same; recording/TC-014 and privacy/TC-003 now read the main call (the watcher's path) and keep `prepare`'s capture for `echoctl capture` / `echoctl transcribe`.
- Stage 2's "row appears by itself" is seen in stage 1 step 2 (the page never reloaded since P1); stage 2 checks the call filling in on the view stage 1 left open. With live transcription the transcript is there about a second after `recording_stopped`, so it is usually already filled in when stage 2 begins.
- Calls are fed from files: several runs can record on one Mac at once, each in its own data folder. The mic falling back to the room after the hang-up (#140) can't happen here: it is the live check's (`recording/TC-023`, person).
- Run 2026-11-08 evening, CLI part only (no browser; window steps checked through `/api/calls`, `/api/call/<name>` and the served `/ipc` where they could be): prelude 16 s, stage 1 38 s (the call held 27 s, `recording_stopped` 13 s after the hang-up), stages 3–7 under 1 s each, stage 8 98 s, stage 9 140 s, stage 10 2 s; every CLI check PASS (the window steps, among them the note typed after the call, not run).
- Run 2026-11-07 on the dev Mac (the old plan: every stage its own call, the local model, the real 15 minutes): stages 1–11 PASS but for #141 and the mic fallback (#140). 2026-11-08 morning, the old plan: about 3 hours; 2026-11-08 afternoon (three TestCall calls, three app starts): 559 s.

---
id: TC-035
title: A call whose transcription failed says so on Today, with why and how to retry, never "transcribing…" for good
priority: medium
type: functional
requirements: [F-TODAY-02]
module: today
run: agent
platform: macos
tags: [today, transcription, recording, file-fed, second-instance]
---

# TC-035: A call whose transcription failed says so on Today, with why and how to retry

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/serve.sh up <dir> <port> --ai fake` (the small set, hermetic); the window is served at `{{base_url}}`; `target/debug/echoctl` is the debug build from this checkout
- **No speech model** in the data's config (`ls "$ECHOCTL_CONFIG_DIR/models"` fails): transcription fails and writes `transcribe.error`
- Calls are fed from files (`app_profile.md` → "Recording with file-fed calls"); every Bash call starts with the prelude: `. <dir>/env; C=tests/fixtures/call.py; W=tests/fixtures/wait.py`
- **Needs a second app instance**, as in `recording/TC-003`; if `<dir>/app.out` says "already running" the case is BLOCKED
- Automatic minutes on **before** the call (a call that ended before they were turned on is not theirs)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes true`, `nohup target/debug/echoctl app run --no-tray --background > <dir>/app.out 2>&1 &`, `$W 30 app` | The app runs |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "NOMODEL-035"; $W 10 armed; $C start acme-sync --hold; $W 20 recording`; after about 10 s `$C end; $W 60 not-recording`; wait 20 s | The call's folder (`…-nomodel-035`) has `mic.wav`, `remote.wav` and `transcribe.error`, no `transcript.md` |
| 3 | Navigate to `{{base_url}}/#today` and read Your day's line of the call (its time; the call is today) | Its reason line says the transcription failed, with why ("no transcription model yet …"), and offers "how to retry"; nowhere on the line "transcribing…" |
| 4 | Wait 60 s and read the line again | Unchanged: still failed, never back to "transcribing…" |
| 5 | Click "how to retry" | The call opens on Minutes, which says transcribing it failed, with the same why and the command to transcribe it again |

## Expected Final State
A call whose transcription failed is never shown as being transcribed: Today says it failed, why, and leads to how to retry.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; `tests/fixtures/serve.sh down <dir>`

## Notes
- From #421 (found on Windows with a memo). A memo takes the same path (Today's minutes line of a call); a memo records the real mic, so this case uses a file-fed call.

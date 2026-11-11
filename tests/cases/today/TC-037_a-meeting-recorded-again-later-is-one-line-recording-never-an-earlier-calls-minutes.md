---
id: TC-037
title: A meeting recorded again later the same day is one line on Your day and one card in Calendar — the recording, saying so — never an earlier call's minutes, never "skipped"
priority: high
type: functional
requirements: [F-TODAY-02, F-CAL-04, F-CAL-07, F-PREP-01]
module: today
run: agent
platform: macos
tags: [today, your-day, recording, file-fed, second-instance]
---

# TC-037: A meeting recorded again later the same day is one line, the recording

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/serve.sh up <dir> <port> --ai fake` (the small set, hermetic) between 06:00 and about 21:30 (the meeting below must stay today); the window is served at `{{base_url}}`; `target/debug/echoctl` is the debug build from this checkout
- The small set holds **Acme weekly sync** recorded about 3 h ago (calendar event `ev-acme-sync`, minutes: 1 decision, 2 actions)
- Calls are fed from files (`app_profile.md` → "Recording with file-fed calls"); every Bash call starts with the prelude: `. <dir>/env; C=tests/fixtures/call.py; W=tests/fixtures/wait.py`
- **Needs a second app instance**, as in `recording/TC-003`; if `<dir>/app.out` says "already running" the case is BLOCKED
- No speech model is needed (the case reads Your day while the call records)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then move the meeting to now, keeping its calendar id (as when the organiser reschedules it): `printf '[{"id":"ev-acme-sync","subject":"Acme weekly sync","start":"%s","end":"%s","response":"accepted","join_url":"https://teams.example/ev-acme-sync"}]' "$(date -u -v-2M +%Y-%m-%dT%H:%M:%SZ)" "$(date -u -v+28M +%Y-%m-%dT%H:%M:%SZ)" \| target/debug/echoctl agenda set -` | `agenda: 1 meeting(s)` |
| 2 | Not recording yet: load `{{base_url}}/`, navigate to `{{base_url}}/#calls` and read the row "Coming up today" | It counts 1 and names "Acme weekly sync" ("Now"): the call held at its old time doesn't make the moved meeting "already a call" |
| 3 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > <dir>/app.out 2>&1 &`, `$W 30 app` | The app runs |
| 4 | Run the prelude, then `target/debug/echoctl arm --title "Acme weekly sync"; $W 10 armed; $C start acme-sync --hold; $W 20 recording`; then `grep -h '"id"' "$ECHOCTL_HOME"/calls/*acme-weekly-sync*/meeting.json` | Recording; both Acme weekly sync folders (the earlier one and the new one) carry `"id": "ev-acme-sync"` |
| 5 | Load `{{base_url}}/` (a full load, not only the hash), then `{{base_url}}/#today`; read Your day's lines of Acme weekly sync | Two lines: the earlier call at its own time (about 3 h ago) with "new minutes · 1 decision · 2 actions", and the meeting at its new time (2 minutes ago) saying "now" and "recording…" — no minutes on it, no "N others at once", no "Show N skipped" |
| 6 | Still recording: navigate to `{{base_url}}/#calendar/day` and wait for "Day"; click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync at its new time) and read its card | The card says "Today · HH:MM–HH:MM · recording" and its line under "Open notes" reads "Recording" — no length (not "Minutes, 30 min", the earlier call's); the earlier call is a block of its own about 3 h ago |
| 7 | Run the prelude, then `$C end; $W 60 not-recording`; load `{{base_url}}/` again, then `#today` | The meeting's line no longer says "recording…" (its call is transcribed — here, with no speech model, "transcription failed" with how to retry); still one line for it, and the earlier call's line unchanged |

## Expected Final State
A meeting recorded again later the same day shows as one line for the recording at its time, and an earlier recording of the same calendar event keeps its own line and minutes.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; `tests/fixtures/serve.sh down <dir>`

## Notes
- From a user's report (Echo 0.9.1): during a recording, Your day linked the meeting to an earlier call of the same event (its minutes on the line) and showed the recording as a line of its own, "skipped · transcribing…"; Calendar's card for the meeting said "recorded" and "Minutes, 31 min" (the earlier call's).
- Navigating to the same URL with only a new hash doesn't reload the page: load `{{base_url}}/` first to read Your day again.

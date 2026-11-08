---
id: TC-010
title: A note added after the call moves the minutes' time
priority: medium
type: functional
requirements: [F-MIN-14, F-NOTES-06]
module: claude
run: agent
platform: macos
tags: [notes, minutes, auto, second-instance, recordings, test-clock]
external_id: TC-CLA-015
---

# TC-010: A note added after the call moves the minutes' time

**Module:** Claude | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake --clock fast` (`claude/TC-003`'s Preconditions: the fake `claude`, automatic minutes **90 s** after the call or the last note in this debug build)
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`); the prelude of the recording cases with `W=tests/fixtures/wait.py`
- The dev app running on the data with automatic minutes on (`claude/TC-003` step 1; in S-17 it is still running from that stage), the window served with clicks reaching the app, the page loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Call | a copy of `minutes-call`, ending when it is copied, title "MIN-B notes move" |
| Note after the call | Ask Kim for the pilot budget |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `CALL="$(python3 tests/fixtures/recordings.py copy minutes-call --title "MIN-B notes move")"; echo "$CALL" > "$D/min-b"; date +%s > "$D/min-b.end"; grep -o '"ended_at": *"[^"]*"' "$CALL/meta.json"` | `ended_at` set (call it E) |
| 2 | Open the call in the window (`{{base_url}}/#calls`, `[role=option][data-call$="min-b-notes-move"]`), Minutes tab | "Minutes in 2 min (HH:MM)" (or "1 min"), HH:MM = E + 90 s |
| 3 | Run the prelude, then `$W 60 sh 'test $(( $(date +%s) - $(cat "$D/min-b.end") )) -ge 35'` (35 s after the call's end: a note within 30 s doesn't count as "after the call"); then open My notes, type `Ask Kim for the pilot budget`, press Enter and click outside the box | The note is listed, "Saved"; the call's `my-notes.md` has it, at N (between E + 35 s and E + 90 s) |
| 4 | Read the Minutes tab (reload the page if it hasn't redrawn) | The time moved to N + 90 s, and the line "Moved: you edited your notes at HH:MM" names the note's time |
| 5 | Run the prelude, then `$W 240 file "$(cat "$D/min-b")/notes-run.json"; cat "$(cat "$D/min-b")/notes-run.json"; stat -f %Sm -t %H:%M:%S "$(cat "$D/min-b")/my-notes.md"` | `"auto": true` and `started_at` no earlier than N + 90 s (the notes' last change + the delay), within 30 s of it — not at E + 90 s |

## Expected Final State
Editing notes after the call postponed the automatic minutes by the delay from the note.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`

## Notes
- Any note more than 30 s after the call's end moves the time (`notes::auto_state`, "moved by notes"); a note within 30 s also sets the time to note + the delay, but the tab doesn't call it moved.
- That the note then is **in** the minutes is what a model writes: `claude/TC-009` (S-19, the local model) has a note made after its call too. Before 2026-11-08 this case waited the real 15 minutes after a 213 s recording and ran the local model.
- Run on 2026-11-07 (S-17, real clock): PASS — call ended 12:39:26, note at 12:41:34, the tab moved from 12:54 to 12:56 PM with "Moved: you edited your notes at 12:41 PM"; the run started 12:56:43 and kept the note.

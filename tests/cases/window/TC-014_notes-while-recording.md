---
id: TC-014
title: Notes during a recording carry the call's time; terminal notes join them
priority: high
type: functional
requirements: [F-NOTES-10]
module: window
run: agent
platform: macos
tags: [notes, recording, second-instance]
external_id: TC-WIN-011
---

# TC-014: Notes during a recording carry the call's time; terminal notes join them

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`), its env feeding calls from files (`recording/TC-001`'s Preconditions); every Bash call starts with the prelude of `recording/TC-001` (`C=tests/fixtures/call.py`, `W=tests/fixtures/wait.py`)
- **A call recording, held on until this case hangs it up.** In S-17: the main call (the scenario's prelude: the app, armed "REC-001 next call", `call.py start acme-sync --hold`), recording when this stage begins; `<slug>` is `rec-001-next-call`. On its own: step 1; `<slug>` is `win-014-notes`
- The window is served on the same test data at `{{base_url}}` (`tests/fixtures/serve.sh up`, or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), recorded on arm, held until step 5 |
| Call slug `<slug>` | `rec-001-next-call` (S-17's main call) or `win-014-notes` (on its own) |
| Note typed | Ask Dana about the Berlin office date |
| Terminal note | Tray-style note from the terminal |
| Note after the call | Checklist goes out Friday |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | On its own: `recording/TC-001` steps 1–2 (the speech model, the app); then run the prelude and `target/debug/echoctl arm --title "WIN-014 notes"; $W 10 armed; $C start acme-sync --hold; $W 20 recording win-014-notes`. In S-17: `$W 5 recording rec-001-next-call` only (the main call is on) | "waited … for recording": `watcher.recording` is a folder ending `-<slug>` |
| 2 | Navigate to `{{base_url}}/#calls`, click the row `[role=option][data-call$="<slug>"]` (the Record pill's title button carries the same `data-call`), open My notes (`[aria-label="My notes"]`) | The call opens on its live transcript; the row and the Transcript tab say Recording; the notes box `[aria-label="New note"]` has the placeholder "Note at m:ss — Enter to add" (the call's clock), a "⚑ Flag" button and the line "The time is taken when you start typing." |
| 3 | Type `Ask Dana about the Berlin office date` in the notes box and press Enter | The note is listed as `mm:ss Ask Dana about the Berlin office date` with the call's time (seconds into the call, e.g. `00:15`), not the wall clock |
| 4 | Type `half a thought` into the notes box without pressing Enter, then run the prelude and `target/debug/echoctl note Tray-style note from the terminal`; then click outside the box on the page (e.g. the transcript's text; not the call's title or a call in the list: opening a call adds the draft as a note) | The command prints "noted in …/my-notes.md". While the box has the focus the list is not redrawn (`typingNotes()` holds it); within 5 s of the click outside, `mm:ss Tray-style note from the terminal` shows in My notes and `half a thought` is still in the box |
| 5 | Hang up once the conversation is over, and wait for the recording to close: run the prelude, then `$W 40 sh '[ $(( $(date +%s) - $(stat -f %m "$ECHOCTL_TEST_CALL_FILE") )) -ge 37 ]'; date +%s > "$D/hangup"; $C end; $W 60 event recording_stopped` | "hung up"; "waited … for event recording_stopped" (10–15 s after the hang-up) |
| 6 | Press Escape in the box (it drops the half-typed draft and its time), type `Checklist goes out Friday` and press Enter | The note is listed without a time (the call is over) and the panel reads "Saved" |
| 7 | Run the prelude, then `cat "$ECHOCTL_HOME"/calls/*-<slug>/my-notes.md` | Three lines in this order: `- [mm:ss] Ask Dana about the Berlin office date`, `- [mm:ss] Tray-style note from the terminal`, `- Checklist goes out Friday` |

## Expected Final State
Notes taken during and after a recording are in one `my-notes.md`, with the call's time where inserted. The call has ended and its recording is closed.

## Teardown
- On its own: run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`; `target/debug/echoctl status`: `watcher` is `null`. In S-17: skipped (the app stays; the window stays on this call for window/TC-013)

## Notes
- Step 4 fails until #276 is fixed: a note added from the terminal while a draft is in the box isn't shown (it is in `my-notes.md`).
- The tray's "Add a note…" is replaced by `echoctl note` (same file, same stamp: `calls::note`); the tray dialog itself is person-run. `echoctl note` works only while a call records ("Error: nothing is being recorded right now" after it): the note after the call is typed in the window.
- The note's time comes from the call's recording clock, so this case needs the app's watcher; with `window --serve` alone the call isn't recording.
- "Time" (insert the call's time) is in My notes' "Edit as text" view (`[aria-label="Edit as text"]`, then `[data-cmd="stamp"]`), shown only while recording; the default view times each note from its first key instead (steps 2–3).
- The call is held (`--hold`) so that the window steps never race its end; step 5 hangs up no earlier than 37 s into it, so the whole conversation is in the recording whatever the steps took.
- Step 6 clears the box with Escape first: a note's time is taken at its first keystroke, so a draft begun during the call and finished after it keeps its time from the call ("The time is taken when you start typing."). That is the app's rule, not a defect; replacing the text without clearing kept `[00:14]` in the 2026-11-08 run.
- Run 2026-11-08 evening (S-17, CLI part only): the terminal note `- [00:19] Tray-style note from the terminal`; the recording closed 13 s after the hang-up. Window steps not run there.
- Run on 2026-11-07 (S-17, TestCall): PASS on the steps then (the Edit-as-text "Time" not exercised).

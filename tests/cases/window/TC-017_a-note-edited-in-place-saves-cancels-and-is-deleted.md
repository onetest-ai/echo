---
id: TC-017
title: A note edited in place changes its text and kind on Enter, Esc cancels, emptied it stays as it was, × deletes it
priority: medium
type: functional
requirements: [F-NOTES-04]
module: window
run: agent
platform: any
data: standard
tags: [notes, edit]
auto: [tests/auto/specs/S-29.spec.mjs]
---

# TC-017: A note edited in place changes its text and kind on Enter, Esc cancels, emptied it stays as it was, × deletes it

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); file reads run after `. <dir>/env`
- Today's Acme weekly sync is open with My notes shown (as in `TC-016`); `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)`
- A note's text opens its editor on click: a textarea `#note-edit` ("Edit this note"), kind buttons Note / ? Question / ! Decision / ☐ To-do / ⚑ Flagged, the hint "Enter to save · Esc to cancel", buttons "Cancel" and "Save". Hovering a note shows ✎ ("Edit this note") and × ("Delete this note")
- **In S-29** the call and My notes are left open by `window/TC-016` (the stage before). On its own: run `TC-016` step 1 first

## Test Data

| Note | Change |
|---|---|
| • 00:12 Dana says the checklist went out on Monday | text → "Dana says the checklist went out on Tuesday", kind → Question |
| ! 00:45 Pilot starts in Berlin only | typed "Pilot starts in Munich", then Esc |
| • a note without a time | emptied, then Save |
| ? 00:31 Does legal need the old contract too | × |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Click the text "Dana says the checklist went out on Monday" | The editor opens with that text; the hint "Enter to save · Esc to cancel" |
| 2 | Replace the text with `Dana says the checklist went out on Tuesday`, click "? Question" in the editor, press Enter; wait 2 s | The note reads "? 00:12 Dana says the checklist went out on Tuesday" (icon "?", class `k-question`); the state "Saved"; `my-notes.md` has `- [00:12] ? Dana says the checklist went out on Tuesday` and no "Monday" |
| 3 | Click "Pilot starts in Berlin only", replace the text with `Pilot starts in Munich`, press Escape; wait 1 s | The editor closes; the note still reads "! 00:45 Pilot starts in Berlin only"; the file still has `- [00:45] ! Pilot starts in Berlin only` |
| 4 | Click "a note without a time", clear the text, click "Save"; wait 2 s | The editor closes; the note still reads "a note without a time" (an emptied edit is dropped; the note is not deleted); the file still has `- a note without a time` |
| 5 | Hover "Does legal need the old contract too" and click its × ("Delete this note"); wait 2 s | The note is gone from the list (6 notes of yours left, the 3 live ✦ rows unchanged); the state "Saved"; the file has no "Does legal need the old contract too" |
| 6 | Reload the page, open the call and My notes again | The list is as after step 5 |

## Expected Final State
One note changed text and kind, one was deleted; the cancelled and the emptied edits changed nothing. `my-notes.md` matches the list.

## Notes
- "Emptied, it is dropped" (`F-NOTES-04`) means the edit is dropped, not the note (`NoteDoc.saveEdit` returns nothing for empty text; `my-notes.test.mjs`: "Emptied: dropped, as Save drops it (delete is the ×)"). An emptied ⚑ flag is the exception: a flag may have no text.
- The data is changed: a scenario reruns on fresh data.

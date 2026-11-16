---
id: TC-026
title: Notes that can't be saved show "Retry", which saves them once they can be; closing the window with unsaved notes asks "Try again / Close without saving"; "Try again" saves and closes
priority: high
type: functional
requirements: [F-NOTES-11]
module: window
run: agent
platform: any
user_steps: ["Opens Echo's own window on the run's data (`echoctl window`), types a note, closes the window with its close button, clicks Try again and Close without saving in the banner (steps 8-13)"]
tags: [notes, files, data-loss, close]
---

# TC-026: Notes that can't be saved show Retry; closing with unsaved notes asks "Try again / Close without saving"

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run: `tests/fixtures/serve.sh up <dir> <port>` (the small set); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); shell steps run after `. <dir>/env`
- **How a save is made to fail (no user-level way):** the call's folder is made read-only (`chmod 555`), so the notes file `my-notes.md` can't be written. Acme weekly sync (today's) has no `my-notes.md` yet. **Steps 2, 6, 9, 12 run `chmod` on that one call folder of the run's data**: the exception to the runner's read-only Bash, on the run's own data only. The Teardown makes it writable again
- **Why the close is a person's part.** Closing the window is the user's click on the native window's close button; a browser tab has no such window, and its close or reload doesn't go through Echo's close handshake (the notes are saved first, then the window closes). So steps 8–13 are done by a person in Echo's own window (`user_steps`): the agent prepares the folder, the person closes, the agent checks the files. The run is BLOCKED, not failed, if the person is not there
- My notes is open in the right panel on Acme weekly sync (step 1)

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync (today): `A=$(ls "$ECHOCTL_HOME/calls" \| grep -- '-acme-weekly-sync$' \| tail -1)`; its folder `$ECHOCTL_HOME/calls/$A`, no `my-notes.md` |
| Notes | `Send Dana the SSO dates`, `Book the Berlin room`, `Discard this line` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click the row `[data-call$="-acme-weekly-sync"]` (today's), open My notes (`[data-panel-flip="notes"]`) | My notes shows the note box `#note-input` and the state `#notes-state` "Saved"; no error notice |
| 2 | In Bash: `A=$(ls "$ECHOCTL_HOME/calls" \| grep -- '-acme-weekly-sync$' \| tail -1); chmod 555 "$ECHOCTL_HOME/calls/$A"; ls -ld "$ECHOCTL_HOME/calls/$A"` | The folder's mode reads `dr-xr-xr-x` |
| 3 | Type `Send Dana the SSO dates` into `#note-input` and press Enter; wait 3 s | The list shows "Send Dana the SSO dates"; an error notice (`.notice.error`) says the notes couldn't be saved and gives the reason, with a button "Retry"; the state `#notes-state` reads "Couldn't save" with a reason |
| 4 | Run `ls "$ECHOCTL_HOME/calls/$A" \| grep -c my-notes` | `0`: nothing was written |
| 5 | Click "Retry"; wait 3 s | The folder is still read-only, so the notice is there again with "Retry"; the note is still listed |
| 6 | In Bash: `chmod 755 "$ECHOCTL_HOME/calls/$A"`; then click "Retry"; wait 3 s; run `cat "$ECHOCTL_HOME/calls/$A/my-notes.md"` | The notice is gone and `#notes-state` reads "Saved"; `my-notes.md` holds a line with `Send Dana the SSO dates` |
| 7 | In Bash: `chmod 555 "$ECHOCTL_HOME/calls/$A"; ls -ld "$ECHOCTL_HOME/calls/$A"`; then in the served window reload `{{base_url}}/` (it is only a viewer from here on) | The folder's mode reads `dr-xr-xr-x` |
| 8 | **The user does:** with `. <dir>/env` loaded, runs `target/debug/echoctl window` (Echo's own window on this data), opens Acme weekly sync, opens My notes, types `Book the Berlin room` into the note box **without pressing Enter**, and clicks the window's close button | The window does not close. A banner over it says the notes couldn't be saved, with two buttons: "Try again" and "Close without saving". The typed line is not lost (it is listed or still in the box) and the box can still be edited: the user can copy their text |
| 9 | In Bash: `grep -c "Berlin" "$ECHOCTL_HOME/calls/$A/my-notes.md"; chmod 755 "$ECHOCTL_HOME/calls/$A"` | The notes file has no "Berlin" in it (`0`); the folder is writable again |
| 10 | **The user does:** clicks "Try again" in the banner | The notes are saved and then the window closes (it is gone) |
| 11 | In Bash: `cat "$ECHOCTL_HOME/calls/$A/my-notes.md"` | A line with `Send Dana the SSO dates` and a line with `Book the Berlin room` |
| 12 | In Bash: `chmod 555 "$ECHOCTL_HOME/calls/$A"`. **The user does:** runs `target/debug/echoctl window` again, opens Acme weekly sync and My notes, types `Discard this line` without Enter, clicks the window's close button, and in the banner clicks "Close without saving" | The banner appears as in step 8; "Close without saving" closes the window at once |
| 13 | In Bash: `chmod 755 "$ECHOCTL_HOME/calls/$A"; grep -c "Discard this line" "$ECHOCTL_HOME/calls/$A/my-notes.md"; grep -c "Berlin" "$ECHOCTL_HOME/calls/$A/my-notes.md"` | `0` then `1`: the discarded line was not saved, and the saved notes are untouched |

## Expected Final State
Failed saves were reported with a Retry that failed while the folder was read-only and saved once it was writable; the close with unsaved notes kept the window open with "Try again / Close without saving" and kept every typed word; "Try again", with the folder writable, saved both notes into `my-notes.md` and then closed the window; "Close without saving" closed it without saving the new line.

## Teardown
- Run `chmod 755 "$ECHOCTL_HOME/calls/$A"` (a read-only folder can't be removed with the rest of the run's data; this covers a run that stopped before step 9 or 13)
- The person closes any Echo window still open on the data
- Reload `{{base_url}}/` and close the panel
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Covers F-NOTES-11 as the feature map words it: notes that can't be saved show Retry; closing with unsaved notes asks "Try again / Close without saving". That "Try again" saves and then closes is the criterion asserted in steps 10–11; a served window can't show it (no native close), which is why it is a person's step.
- The reason after "Couldn't save" is the OS's wording and is not asserted.
- The close saving a note that was typed but not added, on a folder that works, is the third trigger of `F-NOTES-07` that `window/TC-018` leaves to a person.
- Rewritten 2026-11-11 from F-NOTES-11 and #17 (an earlier draft called the page's close entry from a script, which never closed anything); not yet run.

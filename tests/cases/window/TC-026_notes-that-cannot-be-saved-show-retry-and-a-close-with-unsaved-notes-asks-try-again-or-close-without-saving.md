---
id: TC-026
title: Notes that can't be saved show "Retry"; a close with unsaved notes asks "Try again / Close without saving", and once they can be saved it goes on
priority: high
type: functional
requirements: [F-NOTES-11]
module: window
run: agent
platform: any
tags: [notes, files, data-loss, close]
---

# TC-026: Notes that can't be saved show Retry; closing with unsaved notes asks "Try again / Close without saving"

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run: `tests/fixtures/serve.sh up <dir> <port>` (the small set); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); shell steps run after `. <dir>/env`
- **How a save is made to fail:** the call's folder is made read-only (`chmod 555`), so the file `my-notes.md` can't be written (`calls::save_my_notes`: "Permission denied"). Acme weekly sync (today's) has no `my-notes.md` yet. **Steps 2 and 7 run `chmod` on that one call folder of the run's data**: the exception to the runner's read-only Bash, on the run's own data only. It is undone in step 7 and again in the Teardown
- **The close handshake, in a served window.** The app's close (the window's close button, Cmd+Q, an update) asks the page to save with `window.echo.prepareClose(n, close)` and answers the user from what the page replies (`tray/leaving.rs`). A browser tab has no native window, so steps 6 and 9 call that same entry point from the page with `browser_evaluate` (`n` is the handshake number: 1, then 2; `true` is "a close"). The page's side is checked here: the notes saved first, the banner, its two buttons. That the native window then stays open or closes is the app's half: not reachable here (see Notes)
- My notes is open in the right panel on Acme weekly sync (steps 1–2)

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync (today): `A=$(ls "$ECHOCTL_HOME/calls" \| grep -- '-acme-weekly-sync$' \| tail -1)`; its folder `$ECHOCTL_HOME/calls/$A`, no `my-notes.md` |
| Note | `Send Dana the SSO dates` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click the row `[data-call$="-acme-weekly-sync"]` (today's), open My notes (`[data-panel-flip="notes"]`) | My notes shows the note box `#note-input` and the state `#notes-state` "Saved"; no error notice |
| 2 | In Bash: `A=$(ls "$ECHOCTL_HOME/calls" \| grep -- '-acme-weekly-sync$' \| tail -1); chmod 555 "$ECHOCTL_HOME/calls/$A"; ls -ld "$ECHOCTL_HOME/calls/$A"` | The folder's mode reads `dr-xr-xr-x` |
| 3 | Type `Send Dana the SSO dates` into `#note-input` and press Enter; wait 3 s | The list shows "Send Dana the SSO dates"; an error notice (`.notice.error`) reads "Your notes couldn't be saved: …" and the message contains "Permission denied", with a button "Retry"; the state `#notes-state` reads "Couldn't save: …" with the same message |
| 4 | Run `ls "$ECHOCTL_HOME/calls/$A" \| grep -c my-notes` | `0`: nothing was written |
| 5 | Click "Retry"; wait 3 s | The notice is there again with "Retry" and "Permission denied" (the save was tried and failed again); the note is still listed |
| 6 | Type `Book the Berlin room` into `#note-input` without pressing Enter; run `browser_evaluate` `() => { window.echo.prepareClose(1, true); return true }`; wait 3 s; snapshot | A banner over the window (`.toast.error`) reads "Your notes couldn't be saved: …" (the message contains "Permission denied") with two buttons: "Try again" (`[data-cmd="close-retry"]`) and "Close without saving" (`[data-cmd="close-now"]`). The typed line is not lost: the list now holds "Book the Berlin room" too. The note box is not read-only (`#note-input` has no `readonly`): the user can still copy their text |
| 7 | In Bash: `chmod 755 "$ECHOCTL_HOME/calls/$A"; ls -ld "$ECHOCTL_HOME/calls/$A"` | The folder's mode reads `drwxr-xr-x` |
| 8 | Click "Try again" in the banner | The banner is gone (the page asks the app to close again; a served window has no app to answer, so step 9 plays its answer); the notes are still unsaved: `ls "$ECHOCTL_HOME/calls/$A" \| grep -c my-notes` prints `0` |
| 9 | `browser_evaluate` `() => { window.echo.prepareClose(2, true); return true }`; wait 3 s; snapshot, then run `cat "$ECHOCTL_HOME/calls/$A/my-notes.md"` | No banner and no error notice; `#notes-state` reads "Saved"; `#note-input` has `readonly` (the editors stay read-only while Echo closes); `my-notes.md` holds both notes: a line with `Send Dana the SSO dates` and a line with `Book the Berlin room` |

## Expected Final State
Failed saves were reported with a Retry that tried again and failed again; the close with unsaved notes was refused with the banner "Try again / Close without saving" and kept every typed word; with the folder writable again the next close saved both notes into `my-notes.md` and went on (read-only editors, "Saved").

## Teardown
- Run `chmod 755 "$ECHOCTL_HOME/calls/$A"` (a read-only folder can't be removed with the rest of the run's data; it is already writable after step 7, so this only covers a run that stopped before it)
- Reload `{{base_url}}/` (unfreezes the editors that step 9 made read-only) and close the panel
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Covers the page's half of F-NOTES-11: the notice and "Retry" (`15-my-notes.js`, `notes-retry` → `NotesSync.save`), and the close's banner and buttons (`14-call.js` `toasts`; `99-refresh.js` `prepareClose`; `90-render.js` `close-retry`, `close-now`). The error text after "Couldn't save:" is the OS's: only "Permission denied" is asserted.
- The app's half is person-run, in Echo's own window with a note typed and `chmod 555` on the call's folder: the close button leaves the window open and shows the banner; "Try again" closes once the folder is writable again; "Close without saving" closes the window and the unsaved text is gone. The handshake's logic is Rust-tested in `tray/leaving.rs` (`close_ready` with `ok: false` for a close shows the page's banner, for a quit or an update it asks the user).
- "Close without saving" sends `close_now`, which a served window ignores: not clicked here.
- The close saving a note that was typed but not added, on a folder that works, is the third trigger of `F-NOTES-07` that `window/TC-018` leaves to a person.
- Written 2026-11-11 from the code; not yet run. If step 3's message is not "Permission denied" (a different OS wording), the case needs that line only, not the rest.

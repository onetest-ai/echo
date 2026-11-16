---
id: TC-019
title: An unreadable my-notes.md says so with Show folder; one changed outside Echo while a note is typed is kept beside as my-notes.<HHMMSS>.md and the panel says where
priority: high
type: functional
requirements: [F-NOTES-09]
module: window
run: agent
platform: any
data: standard
tags: [notes, files, data-loss]
auto: [tests/auto/specs/S-29.spec.mjs]
---

# TC-019: An unreadable my-notes.md says so with Show folder; one changed outside Echo while a note is typed is kept beside as my-notes.<HHMMSS>.md and the panel says where

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); shell steps run after `. <dir>/env`
- My notes is open in the right panel
- **This case writes one file in the run's data folder** (step 4, "another editor"): the one exception to the runner's read-only Bash, on the run's own data only
- **In S-29** My notes has been open since `window/TC-016` step 1. On its own: open a call, press `[data-panel-flip="notes"]`, then run step 1

## Test Data

| Call | Folder | my-notes.md |
|---|---|---|
| Acme weekly sync, 14 days before the build | `<the build date − 14>_100000-acme-weekly-sync`, the oldest of the three: `O=$(ls "$ECHOCTL_HOME/calls" \| grep -- '-acme-weekly-sync$' \| head -1)` | unreadable: a folder of that name ("Is a directory") |
| Globex standup (today) | `$(ls "$ECHOCTL_HOME/calls" \| grep -- '-globex-standup$')` | `- [00:20] Kim asks for the pilot dates` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#call/<O>/minutes` (the folder of Test Data) | My notes shows, instead of the list and the box, the error "Your notes for this call (my-notes.md) can't be read: Is a directory (os error 21). Fix or replace the file, and they show here again." and a button "Show folder" (do not press it: it opens Finder on this Mac) |
| 2 | Click the row `[data-call$="-globex-standup"]` | My notes lists "• 00:20 Kim asks for the pilot dates", state "Saved" |
| 3 | Type `Ask Kim who signs the pilot` into the note box (`#note-input`), do not press Enter | State "Not saved yet" |
| 4 | In Bash: `G=$(ls "$ECHOCTL_HOME/calls" \| grep -- '-globex-standup$'); printf -- '- Rewritten in another editor\n' > "$ECHOCTL_HOME/calls/$G/my-notes.md"`; wait 3 s | The list still shows "Kim asks for the pilot dates" (nothing is redrawn while the box has focus); the box still holds `Ask Kim who signs the pilot` |
| 5 | Press Enter in the note box; wait 2 s | A notice "The file was changed outside Echo while you typed: your text is saved, the other version is in my-notes.<HHMMSS>.md." with "OK"; the list shows "Kim asks for the pilot dates" and "Ask Kim who signs the pilot"; state "Saved" |
| 6 | Run `ls "$ECHOCTL_HOME/calls/$G"` and `cat` both notes files | `my-notes.md` holds `- [00:20] Kim asks for the pilot dates` and `- Ask Kim who signs the pilot`; a file `my-notes.<HHMMSS>.md` (the name the notice gave) holds `- Rewritten in another editor` |
| 7 | Click "OK" on the notice | The notice is gone; the list stays |

## Expected Final State
The unreadable notes were reported, not overwritten; the user's notes and the note typed meanwhile won over the outside change, which was kept beside them under the name the notice gave.

## Notes
- Steps 4–6 guard #130: while a note is typed in the box, the window's 2-second re-read leaves the notes as shown, so Enter adds the note to them and the save keeps the outside version beside (before the fix the re-read took the outside version and the earlier notes were lost without a copy). Also covered by `notes-sync.test.mjs` ("an outside change while a note is being typed waits…").
- The copy and its notice are also covered by `calls.rs` (`save_my_notes`) and `notes-sync.test.mjs` ("a copy kept of an outside edit is reported once").
- An outside change that only appends (the tray's "Add a note…") is merged instead, with no copy: not this case.
- `call.notes=conflict kept` is not built into the data (README, Skipped): this case makes it.
